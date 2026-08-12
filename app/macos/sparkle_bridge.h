#pragma once

#include <stdbool.h>

void *superiority_sparkle_create(void *event_sink);
void superiority_sparkle_check(void *controller);
void superiority_sparkle_primary_action(void *controller);
void superiority_sparkle_dismiss(void *controller);
void superiority_sparkle_destroy(void *controller);
void superiority_sparkle_render_release_notes(void *text_view, const char *html);
