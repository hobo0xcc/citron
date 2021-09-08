extern int write(int fd, char *buf, int count);
extern int sleep(int delay);
extern int create_window(char *title, int title_len, int x, int y, int width,
                         int height);
extern int map_window(int window_id, unsigned long vaddr);
extern int sync_window(int window_id);
extern int fork();
extern int wait_exit();
extern int read(int fd, char *buf, int count);
extern int seek(int fd, long offset, int whence);
extern int open(char *path);
extern int execve(char *path);

int main(void) {
  char *msg = "Hello, app!\n";
  write(0, msg, 12);
  return 0;
}