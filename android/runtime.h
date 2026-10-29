#pragma once
#include <cstdint>

extern "C" {
char *blockwork_android_start(const char *root);
char *blockwork_android_call(const char *command, const char *args);
char *blockwork_android_attach();
char *blockwork_android_poll(uint64_t session);
char *blockwork_android_detach(uint64_t session);
void blockwork_android_free(char *value);
}
