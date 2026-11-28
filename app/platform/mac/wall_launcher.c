// The Mac's launcher for the files wall (#4101): ilo starts it in place of an assistant's program.
// It reads its rules from its input, closes every handle but the standard three, puts the rules on
// itself and becomes the program. It links libSystem alone, so nothing of ilo's and no framework
// runs inside the wall before the program does. It writes to no stream: ilo learns how it went
// from the system, as the exec or as one of the exit codes below.
//
// Started as: wall-launcher <program, an absolute path> [its arguments...]
//
// The hand-off, on handle 0, little-endian, read once and to the byte, as what follows it on
// that handle is the program's:
//   "ILOW", u32 version, u32 length of the rest
//   the rules:       u32 length, the text, a zero byte
//   the parameters:  u32 count, then a key and a value for each, each written as the rules are
//   the names kept:  u32 count, then each name of the environment, written as the rules are

#include <errno.h>
#include <fcntl.h>
#include <libproc.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/proc_info.h>
#include <unistd.h>

// The Rust that writes the hand-off holds the same numbers, and a test of its reads these lines.
#define WALL_HANDOFF_VERSION 1
#define WALL_HANDOFF_MAX 16384
#define WALL_PARAMETERS_MAX 32
#define WALL_KEPT_MAX 64

// How it ends when nothing was started. ilo has a sentence for each.
enum {
  NO_RULES = 112,    // the hand-off: missing, short, malformed or over a cap
  NO_HANDLES = 113,  // the handles it was started with couldn't be closed or set aside
  NO_WALL = 114,     // the system refused the rules, or a parameter they use wasn't given
  NO_PROGRAM = 115,  // no program, a relative one, or one that couldn't be started
};

// In no public header: libSystem's, and what Chromium's helpers are walled with.
int sandbox_init_with_parameters(const char *profile, uint64_t flags,
                                 const char *const parameters[], char **error);

#define HEAD 12

static unsigned char head[HEAD];
static unsigned char body[WALL_HANDOFF_MAX - HEAD];
static const char *parameters[2 * WALL_PARAMETERS_MAX + 1];
static const char *names[WALL_KEPT_MAX];
static char *kept[WALL_KEPT_MAX + 1];
static struct proc_fdinfo handles[1024];

// Exactly `count` bytes of handle 0, and never one more.
static int read_all(unsigned char *to, size_t count) {
  while (count > 0) {
    ssize_t got = read(0, to, count);
    if (got < 0 && errno == EINTR) continue;
    if (got <= 0) return 0;
    to += got;
    count -= (size_t)got;
  }
  return 1;
}

static uint32_t number(const unsigned char *at) {
  return (uint32_t)at[0] | (uint32_t)at[1] << 8 | (uint32_t)at[2] << 16 | (uint32_t)at[3] << 24;
}

// Where the reading of the body is, and where it ends.
struct cursor {
  size_t at;
  size_t end;
};

static int take_number(struct cursor *from, uint32_t *number_out) {
  if (from->end - from->at < 4) return 0;
  *number_out = number(body + from->at);
  from->at += 4;
  return 1;
}

// A text of the hand-off, which ends in the one zero byte it holds, or NULL.
static const char *take_text(struct cursor *from) {
  uint32_t length;
  if (!take_number(from, &length)) return NULL;
  if (length >= from->end - from->at) return NULL;
  const char *text = (const char *)body + from->at;
  for (uint32_t i = 0; i < length; i++) {
    if (text[i] == 0) return NULL;
  }
  if (text[length] != 0) return NULL;
  from->at += (size_t)length + 1;
  return text;
}

static int same(const char *a, const char *b) {
  while (*a && *a == *b) {
    a++;
    b++;
  }
  return *a == *b;
}

// Whether `name` can be an environment's: not empty, and no equals sign.
static int a_name(const char *name) {
  if (*name == 0) return 0;
  for (; *name; name++) {
    if (*name == '=') return 0;
  }
  return 1;
}

// Whether `entry`, as the environment holds it, is the value of `name`.
static int named(const char *entry, const char *name) {
  while (*name && *entry == *name) {
    entry++;
    name++;
  }
  return *name == 0 && *entry == '=';
}

// The program's environment: of what the launcher was started with, the names kept and no other.
static void keep(char **environment, uint32_t count) {
  size_t next = 0;
  for (uint32_t i = 0; i < count; i++) {
    for (char **entry = environment; *entry; entry++) {
      if (named(*entry, names[i])) {
        kept[next++] = *entry;
        break;
      }
    }
  }
  kept[next] = NULL;
}

// Closes every handle but 0, 1 and 2, so the program inherits none ilo happened to have open.
static int close_handles(void) {
  // Each round closes what it lists, so a full list means there may be more.
  for (int round = 0; round < 256; round++) {
    int size = proc_pidinfo(getpid(), PROC_PIDLISTFDS, 0, handles, (int)sizeof handles);
    if (size <= 0) return 0;
    int count = size / (int)sizeof handles[0];
    for (int i = 0; i < count; i++) {
      if (handles[i].proc_fd > 2) close(handles[i].proc_fd);
    }
    if ((size_t)size < sizeof handles) return 1;
  }
  return 0;
}

// Puts the rules on this process. The system's library writes what it finds wrong in them to
// handle 2, with their text, and that handle is the program's, which ilo keeps the last of: for
// the call it's the null device.
static int wall(const char *rules) {
  int null = open("/dev/null", O_WRONLY | O_CLOEXEC);
  if (null < 0) return NO_HANDLES;
  int said = fcntl(2, F_DUPFD_CLOEXEC, 3);
  if (said < 0 || dup2(null, 2) < 0) return NO_HANDLES;
  char *error = NULL;
  if (sandbox_init_with_parameters(rules, 0, parameters, &error) != 0) return NO_WALL;
  if (dup2(said, 2) < 0 || close(said) != 0 || close(null) != 0) return NO_HANDLES;
  return 0;
}

int main(int argc, char **argv, char **environment) {
  if (!read_all(head, HEAD)) _exit(NO_RULES);
  if (head[0] != 'I' || head[1] != 'L' || head[2] != 'O' || head[3] != 'W') _exit(NO_RULES);
  if (number(head + 4) != WALL_HANDOFF_VERSION) _exit(NO_RULES);
  uint32_t length = number(head + 8);
  if (length > sizeof body || !read_all(body, length)) _exit(NO_RULES);
  struct cursor from = {0, length};

  const char *rules = take_text(&from);
  if (rules == NULL) _exit(NO_RULES);

  uint32_t count;
  if (!take_number(&from, &count) || count > WALL_PARAMETERS_MAX) _exit(NO_RULES);
  for (uint32_t i = 0; i < 2 * count; i++) {
    parameters[i] = take_text(&from);
    if (parameters[i] == NULL) _exit(NO_RULES);
  }
  parameters[2 * count] = NULL;

  uint32_t kept_count;
  if (!take_number(&from, &kept_count) || kept_count > WALL_KEPT_MAX) _exit(NO_RULES);
  for (uint32_t i = 0; i < kept_count; i++) {
    names[i] = take_text(&from);
    if (names[i] == NULL || !a_name(names[i])) _exit(NO_RULES);
    for (uint32_t earlier = 0; earlier < i; earlier++) {
      if (same(names[earlier], names[i])) _exit(NO_RULES);
    }
  }
  if (from.at != from.end) _exit(NO_RULES);

  // By its whole path, so no PATH of anyone's picks what runs.
  if (argc < 2 || argv[1][0] != '/') _exit(NO_PROGRAM);
  keep(environment, kept_count);
  if (!close_handles()) _exit(NO_HANDLES);
  int refused = wall(rules);
  if (refused != 0) _exit(refused);
  execve(argv[1], argv + 1, kept);
  _exit(NO_PROGRAM);
}
