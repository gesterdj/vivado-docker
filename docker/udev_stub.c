/*
 * libudev stub — no-op udev implementations for Vivado in containers.
 *
 * Vivado's HAPRWebtalkHelper and license manager call
 * udev_enumerate_scan_devices() at startup. Containers have no udev
 * daemon or device database, and the scan can misbehave (crashes such
 * as "realloc(): invalid pointer" / "mremap_chunk(): invalid pointer").
 * This stub provides no-op implementations of the udev functions used
 * by Vivado, returning empty results.
 *
 * Loading the stub disables udev-based hardware discovery inside the
 * container: Vivado cannot enumerate JTAG cables or USB debug probes
 * directly. Use a hw_server on the host (VIVADO_HW_SERVER_URL) for
 * hardware access. Synthesis, simulation, and bitstream generation are
 * unaffected.
 *
 * Usage: LD_PRELOAD=/opt/udev_stub.so vivado ...
 *
 * Build: gcc -shared -fPIC -o udev_stub.so udev_stub.c
 */

#include <stdlib.h>

/* Opaque types — Vivado only uses pointers to these */
struct udev;
struct udev_enumerate;
struct udev_list_entry;

struct udev *udev_new(void) {
    /* Return a non-NULL sentinel so callers don't treat it as failure */
    return (struct udev *)1;
}

struct udev *udev_unref(struct udev *udev) {
    (void)udev;
    return NULL;
}

struct udev_enumerate *udev_enumerate_new(struct udev *udev) {
    (void)udev;
    return (struct udev_enumerate *)1;
}

struct udev_enumerate *udev_enumerate_unref(struct udev_enumerate *enumerate) {
    (void)enumerate;
    return NULL;
}

int udev_enumerate_add_match_subsystem(struct udev_enumerate *enumerate,
                                       const char *subsystem) {
    (void)enumerate;
    (void)subsystem;
    return 0;
}

int udev_enumerate_add_match_property(struct udev_enumerate *enumerate,
                                      const char *property,
                                      const char *value) {
    (void)enumerate;
    (void)property;
    (void)value;
    return 0;
}

int udev_enumerate_scan_devices(struct udev_enumerate *enumerate) {
    (void)enumerate;
    return 0;
}

struct udev_list_entry *udev_enumerate_get_list_entry(
    struct udev_enumerate *enumerate) {
    (void)enumerate;
    /* Return NULL = empty list, no devices found */
    return NULL;
}
