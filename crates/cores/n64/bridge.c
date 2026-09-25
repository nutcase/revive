/* Revive's synchronous libretro host. No copyrighted firmware is embedded. */
#include "libretro.h"
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#if defined(_WIN32)
#define REVIVE_EXPORT __declspec(dllexport)
#else
#define REVIVE_EXPORT __attribute__((visibility("default")))
#endif

#define MAX_WIDTH 1024
#define MAX_HEIGHT 625
#define MAX_AUDIO 32768
static uint8_t pixels[MAX_WIDTH * MAX_HEIGHT * 4];
static int16_t samples[MAX_AUDIO];
static size_t sample_count;
static unsigned width, height, pixel_format;
static uint16_t buttons[2];
static int16_t axes[2][2];
static bool audio_enabled;
/* The upstream system-path scratch buffer is 2048 bytes, including suffixes. */
static char save_directory[1024];
static struct { char key[128]; char value[128]; } options[256];
static size_t option_count;
static bool shutdown_requested;

static void log_message(enum retro_log_level level, const char *format, ...) {
    if (level < RETRO_LOG_WARN) return;
    va_list args;
    va_start(args, format);
    vfprintf(stderr, format, args);
    va_end(args);
}

static bool environment(unsigned cmd, void *data) {
    switch (cmd) {
    case RETRO_ENVIRONMENT_GET_LOG_INTERFACE:
        ((struct retro_log_callback *)data)->log = log_message;
        return true;
    case RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY:
        *(const char **)data = save_directory;
        return true;
    case RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY:
        *(const char **)data = save_directory;
        return true;
    case RETRO_ENVIRONMENT_GET_CAN_DUPE:
        *(bool *)data = true;
        return true;
    case RETRO_ENVIRONMENT_SET_PIXEL_FORMAT: {
        unsigned value = *(enum retro_pixel_format *)data;
        if (value > RETRO_PIXEL_FORMAT_RGB565) return false;
        pixel_format = value;
        return true;
    }
    case RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION:
        *(unsigned *)data = 0;
        return true;
    case RETRO_ENVIRONMENT_SET_VARIABLES: {
        const struct retro_variable *vars = data;
        option_count = 0;
        for (; vars && vars->key && option_count < 256; vars++) {
            const char *value = strchr(vars->value, ';');
            if (!value) continue;
            value++;
            while (*value == ' ') value++;
            size_t len = strcspn(value, "|");
            if (len >= sizeof(options[0].value)) return false;
            snprintf(options[option_count].key, sizeof(options[0].key), "%s", vars->key);
            memcpy(options[option_count].value, value, len);
            options[option_count].value[len] = 0;
            option_count++;
        }
        return true;
    }
    case RETRO_ENVIRONMENT_GET_VARIABLE: {
        struct retro_variable *var = data;
        var->value = NULL;
        if (!strcmp(var->key, "mupen64plus-rdp-plugin")) var->value = "angrylion";
        else if (!strcmp(var->key, "mupen64plus-rsp-plugin")) var->value = "cxd4";
        else if (!strcmp(var->key, "mupen64plus-cpucore")) var->value = "cached_interpreter";
        else if (!strcmp(var->key, "mupen64plus-angrylion-multithread")) var->value = "4";
        else if (!strcmp(var->key, "mupen64plus-alt-map")) var->value = "True";
        else if (!strcmp(var->key, "mupen64plus-pak1")) var->value = "memory";
        else if (!strcmp(var->key, "mupen64plus-pak2")) var->value = "none";
        else if (!strcmp(var->key, "mupen64plus-astick-deadzone")) var->value = "0";
        else for (size_t i = 0; i < option_count; i++) {
            if (!strcmp(var->key, options[i].key)) { var->value = options[i].value; break; }
        }
        return var->value != NULL;
    }
    case RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE:
        *(bool *)data = false;
        return true;
    case RETRO_ENVIRONMENT_GET_INPUT_BITMASKS:
        return true;
    case RETRO_ENVIRONMENT_SET_MESSAGE:
        fprintf(stderr, "N64: %s\n", ((struct retro_message *)data)->msg);
        return true;
    case RETRO_ENVIRONMENT_SET_GEOMETRY:
    case RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS:
    case RETRO_ENVIRONMENT_SET_CONTROLLER_INFO:
    case RETRO_ENVIRONMENT_SET_MEMORY_MAPS:
    case RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME:
        return true;
    case RETRO_ENVIRONMENT_SHUTDOWN:
        shutdown_requested = true;
        return true;
    default:
        return false;
    }
}

static void video(const void *data, unsigned w, unsigned h, size_t pitch) {
    if (!data || w == 0 || h == 0 || w > MAX_WIDTH || h > MAX_HEIGHT) return;
    unsigned bpp = pixel_format == RETRO_PIXEL_FORMAT_XRGB8888 ? 4 : 2;
    if (pitch < w * bpp) return;
    width = w; height = h;
    for (unsigned y = 0; y < h; y++) {
        const uint8_t *row = (const uint8_t *)data + y * pitch;
        for (unsigned x = 0; x < w; x++) {
            uint8_t *out = pixels + (y * w + x) * 4;
            if (bpp == 4) {
                uint32_t p; memcpy(&p, row + x * 4, 4);
                out[0] = p >> 16; out[1] = p >> 8; out[2] = p;
            } else {
                uint16_t p; memcpy(&p, row + x * 2, 2);
                unsigned r, g, b = p & 31;
                if (pixel_format == RETRO_PIXEL_FORMAT_RGB565) {
                    r = (p >> 11) & 31; g = (p >> 5) & 63;
                    out[1] = (g << 2) | (g >> 4);
                } else {
                    r = (p >> 10) & 31; g = (p >> 5) & 31;
                    out[1] = (g << 3) | (g >> 2);
                }
                out[0] = (r << 3) | (r >> 2); out[2] = (b << 3) | (b >> 2);
            }
            out[3] = 255;
        }
    }
}

static size_t audio_batch(const int16_t *data, size_t frames) {
    if (audio_enabled) {
        size_t count = frames > (MAX_AUDIO - sample_count) / 2 ? MAX_AUDIO - sample_count : frames * 2;
        memcpy(samples + sample_count, data, count * sizeof(int16_t));
        sample_count += count;
    }
    return frames;
}
static void audio_sample(int16_t left, int16_t right) {
    int16_t pair[2] = {left, right}; audio_batch(pair, 1);
}
static void input_poll(void) {}
static int16_t input_state(unsigned port, unsigned device, unsigned index, unsigned id) {
    if (port >= 2) return 0;
    if (device == RETRO_DEVICE_ANALOG && index == RETRO_DEVICE_INDEX_ANALOG_LEFT && id < 2) return axes[port][id];
    if (device != RETRO_DEVICE_JOYPAD || index != 0) return 0;
    if (id == RETRO_DEVICE_ID_JOYPAD_MASK) return (int16_t)buttons[port];
    return id < 16 ? (buttons[port] >> id) & 1 : 0;
}

REVIVE_EXPORT bool revive_n64_load(const void *rom, size_t rom_size, const char *save_dir) {
    if (strlen(save_dir) >= sizeof(save_directory)) return false;
    strcpy(save_directory, save_dir);
    memset(pixels, 0, sizeof(pixels));
    memset(buttons, 0, sizeof(buttons));
    memset(axes, 0, sizeof(axes));
    width = 320; height = 240; sample_count = 0;
    pixel_format = RETRO_PIXEL_FORMAT_0RGB1555;
    audio_enabled = true; shutdown_requested = false;
    retro_set_environment(environment);
    retro_set_video_refresh(video);
    retro_set_audio_sample(audio_sample);
    retro_set_audio_sample_batch(audio_batch);
    retro_set_input_poll(input_poll);
    retro_set_input_state(input_state);
    retro_init();
    retro_set_controller_port_device(0, RETRO_DEVICE_JOYPAD);
    retro_set_controller_port_device(1, RETRO_DEVICE_NONE);
    retro_set_controller_port_device(2, RETRO_DEVICE_NONE);
    retro_set_controller_port_device(3, RETRO_DEVICE_NONE);
    struct retro_game_info game = {NULL, rom, rom_size, NULL};
    if (!retro_load_game(&game)) { retro_deinit(); return false; }
    return true;
}
REVIVE_EXPORT void revive_n64_close(void) { retro_unload_game(); retro_deinit(); }
REVIVE_EXPORT bool revive_n64_step(void) { sample_count = 0; retro_run(); return !shutdown_requested; }
REVIVE_EXPORT const uint8_t *revive_n64_pixels(void) { return pixels; }
REVIVE_EXPORT unsigned revive_n64_width(void) { return width; }
REVIVE_EXPORT unsigned revive_n64_height(void) { return height; }
REVIVE_EXPORT const int16_t *revive_n64_audio(void) { return samples; }
REVIVE_EXPORT size_t revive_n64_audio_len(void) { return sample_count; }
REVIVE_EXPORT void revive_n64_clear_audio(void) { sample_count = 0; }
REVIVE_EXPORT void revive_n64_audio_enabled(bool enabled) { audio_enabled = enabled; sample_count = 0; }
REVIVE_EXPORT void revive_n64_button(unsigned port, unsigned id, bool pressed) {
    if (port >= 2 || id >= 16) return;
    if (pressed) buttons[port] |= 1u << id; else buttons[port] &= ~(1u << id);
}
REVIVE_EXPORT double revive_n64_fps(void) {
    struct retro_system_av_info av; retro_get_system_av_info(&av); return av.timing.fps;
}

REVIVE_EXPORT void revive_n64_axis(unsigned port, int16_t x, int16_t y) {
    if (port < 2) { axes[port][0] = x; axes[port][1] = y; }
}

/* Asset-free regression for the coroutine ABI used by retro_run. */
#include "libco.h"
static cothread_t test_parent;
static void test_child(void) {
#if defined(__aarch64__)
    register double value __asm__("d8") = 999.0;
    __asm__ volatile("" : "+w"(value));
#endif
    co_switch(test_parent);
#if defined(__aarch64__)
    __asm__ volatile("" : "+w"(value));
#endif
}
REVIVE_EXPORT bool revive_n64_test_coroutine(void) {
    test_parent = co_active();
    cothread_t child = co_create(65536, test_child);
    if (!child) return false;
#if defined(__aarch64__)
    register double value __asm__("d8") = 123.5;
    __asm__ volatile("" : "+w"(value));
    co_switch(child);
    __asm__ volatile("" : "+w"(value));
    bool ok = value == 123.5;
#else
    co_switch(child);
    bool ok = true;
#endif
    co_delete(child);
    return ok;
}
