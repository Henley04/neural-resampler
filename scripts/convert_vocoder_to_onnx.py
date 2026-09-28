#!/usr/bin/env python3
"""把 PC-NSF-HiFiGAN 的 `.ckpt` 导出为 neural-resampler 可用的 ONNX。

仅在拿不到现成 ONNX 时才需要本脚本。默认配置假设：

    输入 mel  [1, n_mels, frames]   float32
    输入 f0   [1, frames]           float32
    输出 audio [1, 1, samples]      float32

用法::

    python3 scripts/convert_vocoder_to_onnx.py \
        --ckpt pc_nsf_hifigan_44.1k_hop512_128bin_2025.02/model.ckpt \
        --out  models/pc_nsf_hifigan.onnx

需要 `torch`；若只想校验已有 ONNX，用 `resampler info` 即可。
"""

from __future__ import annotations

import argparse
from pathlib import Path

import torch


class PCNsfHifiGANWrapper(torch.nn.Module):
    """把声码器包成固定签名的导出壳，方便确定输入输出节点名。"""

    def __init__(self, model: torch.nn.Module):
        super().__init__()
        self.model = model

    def forward(self, mel: torch.Tensor, f0: torch.Tensor) -> torch.Tensor:
        # mel: [1, n_mels, T] -> 模型习惯的 [1, T, n_mels]
        mel = mel.transpose(1, 2)
        wav = self.model(mel, f0)
        if wav.dim() == 2:
            wav = wav.unsqueeze(1)
        return wav


def load_ckpt(ckpt: Path, device: str = "cpu") -> torch.nn.Module:
    """载入 SingingVocoders / openvpi 风格的 checkpoint。

    不同发布件的键名略有差异，这里做几种常见兜底。
    """
    state = torch.load(ckpt, map_location=device)
    if isinstance(state, dict):
        for key in ("state_dict", "model", "generator", "net"):
            if key in state:
                state = state[key]
                break
    state = {k.removeprefix("module.").removeprefix("generator."): v for k, v in state.items()}

    # 延迟导入，避免未安装 torch 时 --help 也失败
    try:
        from utils.nsf_hifigan import NsfHifiGAN  # type: ignore
    except Exception as exc:  # pragma: no cover - 依赖用户本地代码
        raise SystemExit(
            "无法定位 PC-NSF-HiFiGAN 的模型定义。\n"
            "请把 openvpi/SingingVocoders 的模型模块放到 PYTHONPATH，"
            f"或直接使用现成的 ONNX 导出件。原始错误：{exc}"
        ) from exc

    model = NsfHifiGAN()
    missing, unexpected = model.load_state_dict(state, strict=False)
    if missing:
        print(f"[warn] 缺失权重 {len(missing)} 项，例如 {missing[:3]}")
    if unexpected:
        print(f"[warn] 多余权重 {len(unexpected)} 项，例如 {unexpected[:3]}")
    model.eval()
    return model


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--ckpt", required=True, type=Path, help="模型 checkpoint 路径")
    ap.add_argument("--out", required=True, type=Path, help="输出 ONNX 路径")
    ap.add_argument("--n-mels", type=int, default=128)
    ap.add_argument("--frames", type=int, default=128, help="导出用的帧数（动态轴）")
    ap.add_argument("--opset", type=int, default=17)
    args = ap.parse_args()

    model = load_ckpt(args.ckpt)
    wrapper = PCNsfHifiGANWrapper(model).eval()

    mel = torch.zeros(1, args.n_mels, args.frames)
    f0 = torch.zeros(1, args.frames)

    args.out.parent.mkdir(parents=True, exist_ok=True)
    with torch.no_grad():
        torch.onnx.export(
            wrapper,
            (mel, f0),
            str(args.out),
            input_names=["mel", "f0"],
            output_names=["audio"],
            dynamic_axes={
                "mel": {0: "batch", 2: "frames"},
                "f0": {0: "batch", 1: "frames"},
                "audio": {0: "batch", 2: "samples"},
            },
            opset_version=args.opset,
            do_constant_folding=True,
        )
    print(f"已导出: {args.out}")
    print("提示：用 'resampler info' 检查节点名与形状是否匹配。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
