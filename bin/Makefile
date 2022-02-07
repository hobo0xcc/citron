CFLAGS=-ffreestanding -nostdlib -e main
CC=riscv64-unknown-elf-gcc
LD=riscv64-unknown-elf-ld

ifeq ($(shell uname),Linux)
else
ifeq ($(shell uname),Darwin)
else
SHELL       := pwsh.exe
.SHELLFLAGS := -NoProfile -Command
endif
endif

SRC=$(wildcard *.c)
OBJ=$(SRC:.c=.o)
BIN=$(OBJ:.o=)
comma := ,

all: $(BIN)

$(BIN): $(OBJ)
	$(foreach f,$^,$(eval $(shell $(LD) -o $(f:.o=) $(f) syscall.o --entry main)))

$(OBJ): $(SRC) 
	$(foreach f,$^,$(eval $(shell $(CC) -c $(f) syscall.S)))

clean:
ifeq ($(shell uname),Darwin)
	rm -rf $(OBJ) $(BIN)
else
ifeq ($(shell uname),Linux)
	rm -rf $(OBJ) $(BIN)
else
	new-item -Force -Type File _tmp.txt
	-rm -Recurse -Force $(addsuffix $(comma),$(BIN) $(OBJ)) _tmp.txt
endif
endif
# 	$(foreach f,$(BIN),$(eval $(shell rm -Recurse -Force $(f))))
# 	$(foreach f,$(OBJ),$(eval $(shell rm -Recurse -Force $(f))))