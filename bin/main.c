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

char buf[4096];

int main(void) {
  int width = 300;
  int height = 300;
  int window_id = create_window("window", 6, 10, 10, width, height);
  unsigned long buf_addr = 0x10000000;
  int err = map_window(window_id, buf_addr);
  for (int y = 0; y < height; y++) {
    for (int x = 0; x < width; x++) {
      *((unsigned int *)(buf_addr + (y * width + x) * 4)) = (x + y) * 4;
    }
  }

  sync_window(window_id);
  // int pid = fork();
  // if (pid == 0) {
  //   wait_exit();
  //   char *msg = "Hello, world!\n";
  //   write(0, msg, 14);
  // } else {
  //   // execve("/bin/app");
  //   char *msg = "Goodbye, world!\n";
  //   write(0, msg, 16);
  // }
  return 0;
}
