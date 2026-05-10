#!/usr/bin/env python3
from __future__ import annotations

import sys


def main():
    import numpy
    import torch
    import torchaudio
    import torchvision

    if not hasattr(torch, "_utils"):
        raise RuntimeError("torch._utils 缺失，当前 PyTorch 安装不完整。")

    numpy_major = int(numpy.__version__.split(".", 1)[0])
    if numpy_major >= 2:
        raise RuntimeError(
            f"NumPy 版本不兼容，当前为 {numpy.__version__}，需要使用 numpy<2。"
        )

    try:
        torch.zeros(1).numpy()
    except Exception as error:
        raise RuntimeError(f"PyTorch 与 NumPy 集成不可用：{error}") from error

    print(f"numpy={numpy.__version__}")
    print(f"torch={torch.__version__}")
    print(f"torchvision={torchvision.__version__}")
    print(f"torchaudio={torchaudio.__version__}")
    print("torch runtime validation passed.")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        sys.stderr.write(f"{error}\n")
        sys.exit(1)
