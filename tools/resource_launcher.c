/* Linux-only measurement helper, never linked into the compiler or VM.
 * Fork after exec so the measured child's RSS excludes Python's pre-exec peak.
 */
#define _GNU_SOURCE
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr, "Usage: resource-launcher METRICS_FILE MEMORY_MIB PROGRAM [ARG...]\n");
        return 125;
    }
    char *end = NULL;
    errno = 0;
    unsigned long memory = strtoul(argv[2], &end, 10);
    if (errno || !*argv[2] || *end || memory == 0 || memory > 4096) {
        fprintf(stderr, "Invalid memory limit\n");
        return 125;
    }
    pid_t child = fork();
    if (child == -1) {
        perror("fork");
        return 125;
    }
    if (child == 0) {
        struct rlimit core = {0, 0};
        rlim_t bytes = (rlim_t)memory * 1024 * 1024;
        struct rlimit address_space = {bytes, bytes};
        if (setrlimit(RLIMIT_CORE, &core) || setrlimit(RLIMIT_AS, &address_space)) {
            perror("setrlimit");
            _exit(125);
        }
        execv(argv[3], &argv[3]);
        perror("execv");
        _exit(125);
    }
    int status = 0;
    struct rusage usage;
    while (wait4(child, &status, 0, &usage) == -1) {
        if (errno == EINTR) {
            continue;
        }
        perror("wait4");
        return 125;
    }
    FILE *metrics = fopen(argv[1], "w");
    if (!metrics) {
        perror("metrics fopen");
        return 125;
    }
    int exit_code = WIFEXITED(status) ? WEXITSTATUS(status) : -WTERMSIG(status);
    int write_result = fprintf(metrics,
                "{\"process_exit_code\":%d,\"peak_rss_kib\":%ld,"
                "\"user_seconds\":%.6f,\"system_seconds\":%.6f}\n",
                exit_code, usage.ru_maxrss,
                usage.ru_utime.tv_sec + usage.ru_utime.tv_usec / 1000000.0,
                usage.ru_stime.tv_sec + usage.ru_stime.tv_usec / 1000000.0);
    int close_result = fclose(metrics);
    if (write_result < 0 || close_result != 0) {
        perror("metrics write");
        return 125;
    }
    return exit_code < 0 ? 128 - exit_code : exit_code;
}
