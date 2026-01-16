DOCKER_TAG ?= comix:latest

# 架构选择: riscv (默认) 或 loongarch
ARCH ?= riscv

# 根据架构设置 target 和运行命令
ifeq ($(ARCH),loongarch)
    TARGET := loongarch64-unknown-none
    TARGET_DIR := target/loongarch64-unknown-none/debug
    PROJECT_DIR := $(TARGET_DIR)/os
    RUN_SCRIPT := cargo run --target $(TARGET)
    GDB_SCRIPT := cargo run --target $(TARGET) -- gdb
else
    TARGET := riscv64gc-unknown-none-elf
    RUN_SCRIPT := cargo run
    GDB_SCRIPT := cargo run -- --gdb
endif

.PHONY: docker build_docker fmt run build clean clean-all gdb

docker:
	docker run --rm -it -v ${PWD}:/mnt -w /mnt --name comix ${DOCKER_TAG} bash

build_docker:
	docker build -t ${DOCKER_TAG} --target build .

fmt:
	cd os && cargo fmt

# 构建内核（build.rs 会自动编译 user 并打包镜像）
build:
	cd os && cargo build --target $(TARGET)

# 运行内核（build.rs 会自动编译 user 并打包镜像）
run:
	cd os && $(RUN_SCRIPT)

# GDB 调试模式
gdb:
	cd os && $(GDB_SCRIPT)

# 清理 OS 构建产物
clean:
	cd os && cargo clean

# 清理所有构建产物（包括 user）
clean-all: clean
	cd user && make clean

# 手动编译用户程序（通常不需要，build.rs 会自动处理）
build-user:
	cd user && make

# 帮助信息
help:
	@echo "ComixOS Makefile"
	@echo ""
	@echo "Usage: make [target] ARCH=[riscv|loongarch]"
	@echo ""
	@echo "Architectures:"
	@echo "  riscv      - RISC-V 64-bit (default)"
	@echo "  loongarch  - LoongArch 64-bit"
	@echo ""
	@echo "Targets:"
	@echo "  build      - Build the kernel"
	@echo "  run        - Run the kernel in QEMU"
	@echo "  gdb        - Run with GDB debugging"
	@echo "  clean      - Clean build artifacts"
	@echo "  help       - Show this help message"
	@echo ""
	@echo "Examples:"
	@echo "  make build                    # Build for RISC-V"
	@echo "  make build ARCH=loongarch     # Build for LoongArch"
	@echo "  make run ARCH=loongarch       # Run LoongArch kernel"