// Fixture for crates/rbqn/tests/ffi.rs: one function per FFI type class.
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

int8_t   t_i8 (int8_t x)   { return x + 1; }
int16_t  t_i16(int16_t x)  { return x + 1; }
int32_t  t_i32(int32_t x)  { return x + 1; }
int64_t  t_i64(int64_t x)  { return x + 1; }
uint8_t  t_u8 (uint8_t x)  { return x + 1; }
uint16_t t_u16(uint16_t x) { return x + 1; }
uint32_t t_u32(uint32_t x) { return x + 1; }
uint64_t t_u64(uint64_t x) { return x + 1; }
float    t_f32(float x)    { return x * 2; }
double   t_f64(double x)   { return x * 2; }

// Sums of `*type` lists, one per numeric type.
#define SUM(N, T, R) R sum_##N(const T *p, int32_t n) { R s = 0; for (int32_t i = 0; i < n; i++) s += p[i]; return s; }
SUM(i8, int8_t, int64_t)   SUM(i16, int16_t, int64_t) SUM(i32, int32_t, int64_t) SUM(i64, int64_t, int64_t)
SUM(u8, uint8_t, uint64_t) SUM(u16, uint16_t, uint64_t) SUM(u32, uint32_t, uint64_t) SUM(u64, uint64_t, uint64_t)
SUM(f32, float, double)    SUM(f64, double, double)

// & mutable buffers.
void double_in_place(int32_t *p, int32_t n) { for (int32_t i = 0; i < n; i++) p[i] *= 2; }
int32_t fill_and_count(double *p, int32_t n) { for (int32_t i = 0; i < n; i++) p[i] = i + 0.5; return n; }
void upcase(char *s) { for (; *s; s++) if (*s >= 'a' && *s <= 'z') *s -= 32; }

// Strings and raw data (:c8, :u8 conversions).
const char *greeting(void) { return "hello from C"; }
int32_t byte_sum(const uint8_t *p, int32_t n) { int32_t s = 0; for (int32_t i = 0; i < n; i++) s += p[i]; return s; }
const uint32_t *words(void) { static const uint32_t w[] = {1, 2, 3, 0}; return w; }

// BQN values are passed as opaque 64-bit words.
uint64_t pass_a(uint64_t x) { return x; }

// Structs by value, nested structs, arrays in structs.
typedef struct { int32_t a; double b; } Pair;
Pair    make_pair(int32_t a, double b) { Pair p = {a, b}; return p; }
double  pair_sum(Pair p) { return p.a + p.b; }
typedef struct { uint8_t tag; int16_t xs[3]; Pair in; } Nest;
int32_t nest_sum(Nest n) { return n.tag + n.xs[0] + n.xs[1] + n.xs[2] + n.in.a + (int32_t)n.in.b; }
Nest    make_nest(void) { Nest n = {7, {1, 2, 3}, {40, 2.0}}; return n; }
typedef struct { double x, y; } Vec2;
Vec2    vec_add(Vec2 a, Vec2 b) { Vec2 r = {a.x + b.x, a.y + b.y}; return r; }

// Pointer objects: heap pairs and arrays.
Pair   *new_pairs(int32_t n) { Pair *p = calloc(n, sizeof(Pair)); for (int32_t i = 0; i < n; i++) { p[i].a = i; p[i].b = i * 10; } return p; }
int32_t pair_a(const Pair *p) { return p->a; }
int32_t *new_ints(int32_t n) { int32_t *p = malloc(n * sizeof *p); for (int32_t i = 0; i < n; i++) p[i] = 100 + i; return p; }
int32_t first_int(const int32_t *p) { return *p; }
void    free_ptr(void *p) { free(p); }

// qsort comparator, so a `&` buffer can be sorted by libc.
static int cmp_i32(const void *a, const void *b) { int32_t x = *(const int32_t *)a, y = *(const int32_t *)b; return (x > y) - (x < y); }
void *get_cmp(void) { return (void *)cmp_i32; }
