int write(int fd, char *buf, int count);
int sleep(int delay);
int create_window(char *title, int title_len, int x, int y, int width,
                  int height);
int map_window(int window_id, unsigned long vaddr);
int sync_window(int window_id);
int fork();
int wait_exit();
int read(int fd, char *buf, int count);
int seek(int fd, long offset, int whence);
int open(char *path);
int execve(char *path);