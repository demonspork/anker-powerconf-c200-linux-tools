#include "awcap.h"

#define _DEFAULT_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/time.h>
#include <time.h>
#include <sys/mman.h>
#include <sys/select.h>
#include <unistd.h>
#include <linux/videodev2.h>

#define AWCAP_BUFFERS 4

struct awcap_buffer {
	void *start;
	size_t length;
};

struct awcap {
	int fd;
	uint32_t width;
	uint32_t height;
	uint32_t fourcc;
	unsigned int buffer_count;
	struct awcap_buffer buffers[AWCAP_BUFFERS];
	int have_frame;
	struct v4l2_buffer current;
};

static int xioctl(int fd, unsigned long request, void *arg)
{
	int result = 0;

	do {
		result = ioctl(fd, request, arg);
	} while (result < 0 && errno == EINTR);

	return result;
}

int awcap_list_modes(const char *path, struct awcap_mode *out, int max)
{
	int fd = open(path, O_RDWR);
	int count = 0;
	struct v4l2_fmtdesc fmt;

	if (fd < 0) {
		return -1;
	}

	memset(&fmt, 0, sizeof(fmt));
	fmt.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
	for (fmt.index = 0; xioctl(fd, VIDIOC_ENUM_FMT, &fmt) == 0; ++fmt.index) {
		struct v4l2_frmsizeenum size;

		memset(&size, 0, sizeof(size));
		size.pixel_format = fmt.pixelformat;
		for (size.index = 0; xioctl(fd, VIDIOC_ENUM_FRAMESIZES, &size) == 0; ++size.index) {
			struct v4l2_frmivalenum ival;
			uint32_t max_fps = 0;

			if (size.type != V4L2_FRMSIZE_TYPE_DISCRETE) {
				continue;
			}

			memset(&ival, 0, sizeof(ival));
			ival.pixel_format = fmt.pixelformat;
			ival.width = size.discrete.width;
			ival.height = size.discrete.height;
			for (ival.index = 0; xioctl(fd, VIDIOC_ENUM_FRAMEINTERVALS, &ival) == 0; ++ival.index) {
				if (ival.type == V4L2_FRMIVAL_TYPE_DISCRETE && ival.discrete.numerator != 0) {
					uint32_t fps = ival.discrete.denominator / ival.discrete.numerator;

					if (fps > max_fps) {
						max_fps = fps;
					}
				}
			}

			if (count < max) {
				out[count].fourcc = fmt.pixelformat;
				out[count].width = size.discrete.width;
				out[count].height = size.discrete.height;
				out[count].max_fps = max_fps;
			}
			++count;
		}
	}

	close(fd);
	return count < max ? count : max;
}

static void set_error(char *err, size_t err_len, const char *stage)
{
	if (err != NULL && err_len > 0) {
		snprintf(err, err_len, "%s", stage);
	}
}

struct awcap *awcap_open(const char *path, uint32_t fourcc, uint32_t width, uint32_t height, uint32_t fps, char *err, size_t err_len)
{
	struct awcap *cap = calloc(1, sizeof(*cap));
	struct v4l2_format format;
	struct v4l2_requestbuffers request;
	enum v4l2_buf_type type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
	unsigned int i = 0;

	if (cap == NULL) {
		set_error(err, err_len, "allocate");
		return NULL;
	}

	cap->fd = open(path, O_RDWR);
	if (cap->fd < 0) {
		set_error(err, err_len, "open");
		free(cap);
		return NULL;
	}

	memset(&format, 0, sizeof(format));
	format.type = type;
	format.fmt.pix.width = width;
	format.fmt.pix.height = height;
	format.fmt.pix.pixelformat = fourcc;
	format.fmt.pix.field = V4L2_FIELD_ANY;
	if (xioctl(cap->fd, VIDIOC_S_FMT, &format) < 0) {
		set_error(err, err_len, "set format");
		goto fail;
	}
	cap->width = format.fmt.pix.width;
	cap->height = format.fmt.pix.height;
	cap->fourcc = format.fmt.pix.pixelformat;

	if (fps > 0) {
		struct v4l2_streamparm parm;

		memset(&parm, 0, sizeof(parm));
		parm.type = type;
		parm.parm.capture.timeperframe.numerator = 1;
		parm.parm.capture.timeperframe.denominator = fps;
		/* Best effort: some drivers reject this, the stream still works. */
		xioctl(cap->fd, VIDIOC_S_PARM, &parm);
	}

	memset(&request, 0, sizeof(request));
	request.count = AWCAP_BUFFERS;
	request.type = type;
	request.memory = V4L2_MEMORY_MMAP;
	if (xioctl(cap->fd, VIDIOC_REQBUFS, &request) < 0 || request.count == 0) {
		set_error(err, err_len, "request buffers");
		goto fail;
	}
	cap->buffer_count = request.count < AWCAP_BUFFERS ? request.count : AWCAP_BUFFERS;

	for (i = 0; i < cap->buffer_count; ++i) {
		struct v4l2_buffer buffer;

		memset(&buffer, 0, sizeof(buffer));
		buffer.type = type;
		buffer.memory = V4L2_MEMORY_MMAP;
		buffer.index = i;
		if (xioctl(cap->fd, VIDIOC_QUERYBUF, &buffer) < 0) {
			set_error(err, err_len, "query buffer");
			goto fail;
		}
		cap->buffers[i].length = buffer.length;
		cap->buffers[i].start = mmap(NULL, buffer.length, PROT_READ | PROT_WRITE, MAP_SHARED, cap->fd, buffer.m.offset);
		if (cap->buffers[i].start == MAP_FAILED) {
			cap->buffers[i].start = NULL;
			set_error(err, err_len, "mmap");
			goto fail;
		}
		if (xioctl(cap->fd, VIDIOC_QBUF, &buffer) < 0) {
			set_error(err, err_len, "queue buffer");
			goto fail;
		}
	}

	if (xioctl(cap->fd, VIDIOC_STREAMON, &type) < 0) {
		set_error(err, err_len, "stream on");
		goto fail;
	}

	return cap;

fail:
	{
		int saved = errno;

		awcap_close(cap);
		errno = saved;
	}
	return NULL;
}

uint32_t awcap_width(const struct awcap *cap)
{
	return cap->width;
}

uint32_t awcap_height(const struct awcap *cap)
{
	return cap->height;
}

uint32_t awcap_fourcc(const struct awcap *cap)
{
	return cap->fourcc;
}

int awcap_wait_frame(struct awcap *cap, int timeout_ms, const uint8_t **data, size_t *len)
{
	fd_set fds;
	struct timeval tv;
	int result = 0;

	if (cap->have_frame) {
		awcap_release_frame(cap);
	}

	FD_ZERO(&fds);
	FD_SET(cap->fd, &fds);
	tv.tv_sec = timeout_ms / 1000;
	tv.tv_usec = (timeout_ms % 1000) * 1000;
	do {
		result = select(cap->fd + 1, &fds, NULL, NULL, &tv);
	} while (result < 0 && errno == EINTR);
	if (result < 0) {
		return -1;
	}
	if (result == 0) {
		return 0;
	}

	memset(&cap->current, 0, sizeof(cap->current));
	cap->current.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
	cap->current.memory = V4L2_MEMORY_MMAP;
	if (xioctl(cap->fd, VIDIOC_DQBUF, &cap->current) < 0) {
		if (errno == EAGAIN) {
			return 0;
		}
		return -1;
	}
	if (cap->current.index >= cap->buffer_count) {
		errno = EIO;
		return -1;
	}

	cap->have_frame = 1;
	if (cap->current.bytesused == 0 || (cap->current.flags & V4L2_BUF_FLAG_ERROR) != 0) {
		/* Corrupt or empty frame: hand it back and report a timeout-like miss. */
		awcap_release_frame(cap);
		return 0;
	}

	*data = cap->buffers[cap->current.index].start;
	*len = cap->current.bytesused;
	return 1;
}

int awcap_release_frame(struct awcap *cap)
{
	if (!cap->have_frame) {
		return 0;
	}
	cap->have_frame = 0;
	return xioctl(cap->fd, VIDIOC_QBUF, &cap->current);
}

void awcap_close(struct awcap *cap)
{
	enum v4l2_buf_type type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
	unsigned int i = 0;

	if (cap == NULL) {
		return;
	}
	if (cap->fd >= 0) {
		xioctl(cap->fd, VIDIOC_STREAMOFF, &type);
	}
	for (i = 0; i < AWCAP_BUFFERS; ++i) {
		if (cap->buffers[i].start != NULL) {
			munmap(cap->buffers[i].start, cap->buffers[i].length);
		}
	}
	if (cap->fd >= 0) {
		close(cap->fd);
	}
	free(cap);
}

int awctl_query(int fd, uint32_t id, struct awctl_info *out)
{
	struct v4l2_queryctrl query;

	memset(&query, 0, sizeof(query));
	query.id = id;
	if (xioctl(fd, VIDIOC_QUERYCTRL, &query) < 0) {
		return -1;
	}
	if ((query.flags & V4L2_CTRL_FLAG_DISABLED) != 0) {
		errno = ENOTSUP;
		return -1;
	}

	out->minimum = query.minimum;
	out->maximum = query.maximum;
	out->step = query.step;
	out->default_value = query.default_value;
	out->flags = query.flags;
	out->type = query.type;
	return 0;
}

int awctl_get(int fd, uint32_t id, int32_t *value)
{
	struct v4l2_control control;

	memset(&control, 0, sizeof(control));
	control.id = id;
	if (xioctl(fd, VIDIOC_G_CTRL, &control) < 0) {
		return -1;
	}
	*value = control.value;
	return 0;
}

int awctl_set(int fd, uint32_t id, int32_t value)
{
	struct v4l2_control control;

	memset(&control, 0, sizeof(control));
	control.id = id;
	control.value = value;
	return xioctl(fd, VIDIOC_S_CTRL, &control);
}

int awctl_menu_name(int fd, uint32_t id, uint32_t index, char *name, size_t name_len)
{
	struct v4l2_querymenu menu;

	memset(&menu, 0, sizeof(menu));
	menu.id = id;
	menu.index = index;
	if (xioctl(fd, VIDIOC_QUERYMENU, &menu) < 0) {
		return -1;
	}
	snprintf(name, name_len, "%s", (const char *)menu.name);
	return 0;
}
