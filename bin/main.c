extern int write(int fd, char *buf, int count);
extern int sleep(int delay);

int main(void)
{
    // while (1)
    // {
    //     write(0, "Hello\n", 6);
    //     sleep(100);
    // }
    char *msg = "Hello, world!\n";
    write(0, msg, 14);
    return 0;
}