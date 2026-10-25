#ifndef SUA_VIEW_H
#define SUA_VIEW_H
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct SuaBook SuaBook;

/* Serialize every call using a given book. No call may race close.
 * Returned buffers and strings are owned by the caller and must use the
 * matching free function. Errors are UTF-8 strings; success sets *error NULL.
 * Paths are UTF-8. Indices are zero-based. */
SuaBook *sua_book_open(const char *path, char **error);
size_t sua_book_count(const SuaBook *book);
size_t sua_book_initial_page(const SuaBook *book);
char *sua_book_page_name(const SuaBook *book, size_t index);
uint8_t *sua_book_read_page(SuaBook *book, size_t index, size_t *length, char **error);
void sua_bytes_free(uint8_t *bytes, size_t length);
void sua_string_free(char *text);
void sua_book_close(SuaBook *book);

/* Shared reading policy. Boolean parameters/results are 0/1 bytes, not a
 * language-specific bool. end is exclusive (book count or previous span end).
 * Aspect states for previous_start: -1 unknown, 0 portrait, 1 landscape.
 * These pure helpers may be called without a book or IO lock. */
uint8_t sua_page_is_landscape(double width, double height);
uint8_t sua_can_pair(size_t index, size_t end, uint8_t spread, uint8_t cover,
                     uint8_t first_landscape, uint8_t second_landscape);
size_t sua_next_start(size_t index, size_t count, size_t displayed);
size_t sua_previous_start(size_t index, uint8_t spread, uint8_t cover,
                          int32_t previous_landscape, int32_t earlier_landscape);
uint8_t sua_path_is_image(const char *path);

#ifdef __cplusplus
}
#endif
#endif
