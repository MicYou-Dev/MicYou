# 纯 Rust 推理移植:PureVox6 / AEC7 去 ONNX Runtime 化

> 状态:已在实验性后端 [libmicyou](https://github.com/OrientCOMPASS/libmicyou)
> 完成开发、验证与性能对比,并移植回本仓库。方法论对齐
> [silero_v4 纯 Rust 移植](https://github.com/OrientCOMPASS/Qwen3-subtitle-assistant/blob/master/finetune/silero_v4_port.md)
> (numpy 参照实现 → 位精确对拍 → 权重平面化 → Rust 机械转写 → golden 测试),
> 并针对这两个**大得多、且以匿名算子图导出**的模型做了工程化推广:
> 参照实现是通用的 numpy ONNX 解释器,Rust 侧是静态编译产物 + 微型 VM。
>
> **两仓库分工**:模型生产与验证工具链(numpy 解释器/编译器、ONNX 源模型、
> golden fixtures、对拍 CI)全部留在 libmicyou;本仓库只包含**运行时产物**——
> 零依赖 VM(`tauri-app/crates/micyou-infer`)与编译好的 MCYI 权重 blob
> (`tauri-app/crates/micyou-audio/src/assets/*.mcy`,`include_bytes!` 嵌入)。

## 1. 为什么可行(评估结论)

对 `purevox6.onnx`(2.1 MB,1105 节点)与 `aec7_ep0185.onnx`(4.8 MB,1581 节点)的
图分析显示:

| 维度 | purevox6 | aec7 | 结论 |
|---|---|---|---|
| 算子种类 | 28 | 30 | 全部可用 numpy/Rust 机械实现(Conv/ConvT/GRU/LN/BN/Resize/…) |
| 动态 shape | 无(batch=1,全部静态) | 无 | 可全静态编译,VM 无需 shape 推理 |
| 结构性算子 | Slice/Reshape/Pad/Expand 的索引全部来自 Constant/Shape 链 | 同左 | 编译期常量折叠,运行时零 int64 张量 |
| 每帧计算量 | **2.75 MMAC**(GRU 1.71M 为主) | **8.4 MMAC**(GRU 4.25M 为主) | 10 ms 帧预算内:即使标量 1 MAC/cycle 也只要 0.9/2.8 ms |
| 折叠后运行时节点 | 584 → 440 条指令 | 802 → 616 条指令 | 别名化(Reshape/Squeeze 零成本)+ 死代码消除 |
| 运行时内存 | arena 0.14 MB + 权重 2.06 MB | arena 1.85 MB + 权重 4.73 MB | 活跃区间打包后极小 |

关键风险点均已验证消除:GRU(含双向、linear_before_reset)、ConvTranspose
(output_padding)、Resize(sizes 驱动、half_pixel 线性)、ONNX Slice 负步长语义
(`end=-1` = 越过 0 而非 numpy 的末元素)——numpy 参照实现与 onnxruntime 的
流式对拍最大绝对误差 ~1e-6(f32 求和序噪声级)。

## 2. 架构

```
libmicyou 仓库(模型生产与验证,不进本仓库):
  purevox6.onnx / aec7_ep0185.onnx      (权重来源与 CI 对拍基准)
        │  tools/onnx-port/compile_models.py
        │    ① numpy 解释器全图 trace(所有 shape/常量值)
        │    ② 常量折叠(Constant/Shape/结构性链全部编译期求值)
        │    ③ Reshape/Squeeze/Unsqueeze/Identity → slot 别名(零指令)
        │    ④ 死代码消除 + 活跃区间打包 f32 arena
        ▼
  {purevox6,aec7}.mcy                   (MCYI 平面二进制:指令流 + slot 表
        │                                + f32 权重;格式确定性,CI 新鲜度门禁)
        │  提交到本仓库
        ▼
本仓库(运行时):
  tauri-app/crates/micyou-audio/src/assets/*.mcy
        │  include_bytes! 嵌入二进制
        ▼
  tauri-app/crates/micyou-infer         (零依赖纯 Rust VM:~26 个定形算子内核,
        │                                预打包 arena,每帧零分配、零图解析)
        ▼
  micyou-audio dsp.rs                   (PureVoxProcessor / AecProcessor:
                                         STFT/OLA 布线不变,ort Session → VM Session)
```

`ort` crate、`libonnxruntime.{so,dll,dylib}` 动态加载与分发、`.onnx` 模型文件发现
逻辑全部移除;运行时资源目录只剩 PipeWire/ALSA 配置与模型许可证。模型不可再
"缺失":编译进二进制,降噪/AEC 永远可用。

**Intel Mac 解锁**:旧方案捆绑的 `libonnxruntime.dylib` 是 arm64-only thin
binary(非 universal),这是桌面端此前只发布 macOS arm64 安装包、Intel Mac
用不上 AI 降噪/AEC 的直接原因。纯 Rust VM 没有这个约束:x86_64 路径带
AVX2+FMA 运行时派发(`is_x86_feature_detected`,不满足时回落 baseline 标量
路径),零 C 依赖可在 arm64 runner 上直接交叉编译——CI(development /
release / pre-release)随之产出 `x86_64-apple-darwin` dmg,MirrorChyan 上传
清单同步增加 macos/x64 条目。

## 3. MCYI blob 格式(v1,小端)

```
magic "MCYI" | version u32 | n_slots n_ops n_in n_out u32 | arena_len const_len u32
input slot ids | output slot ids
slot 表(长度前缀): kind u8 (0=input 1=const 2=arena 3=alias) | ndim u8 | pad u16
                    | off i32(const:f32 偏移;alias:目标 slot;input:绑定序号)
                    | dims i32×ndim
op 表(长度前缀): code u16 | n_i64 n_in n_out n_f32 u16 | in/out slot ids u32
                  | i64 attrs | f32 attrs
const 数据段: f32 × const_len
```

算子属性布局(与 libmicyou `onnxref/compiler.py` 一一对应):

| opcode | i64 attrs | 备注 |
|---|---|---|
| SLICE(18) | `[n, (axis,start,end,step)×n]` | 负步长 `end=-1`=到 0 为止;`(0,0)`=空选择 |
| PAD(19) | `[begin×nd, end×nd]` + f32 `[cv]` | |
| CONV(11)/CONV_T(12) | `[kh,kw,sh,sw,dh,dw,pt,pl,pb,pr,group,has_bias(,opt_h,opt_w)]` | NCHW;1×1→GEMM、depthwise→直算、其余→im2col |
| GRU(13) | `[hidden, direction(0f/1b/2bi), lbr]` | 输入 X,W,R,B?,H0?(NULL_SLOT=缺省);输出 [Y, Y_h] 恒两位 |
| LAYER_NORM(15) | `[axis]` + f32 `[eps]` | opset-17 语义(axis 起全部归一) |
| BATCH_NORM(14) | f32 `[eps]` | 推理模式 |
| REDUCE_MEAN/L2(22/23) | `[keepdims, n_axes, axes…]` | |
| GATHER(21) | `[axis, idx…]` | 常量索引 |
| CLIP(9) | `[has_lo, has_hi]` + f32 `[lo, hi]` | |
| RESIZE(24) | 无 | linear + half_pixel,目标尺寸=输出 slot shape |
| 其余 | 见 compiler.py | ADD/SUB/MUL/DIV/POW/SIGMOID/SQRT/LOG/MATMUL/TRANSPOSE/CONCAT/EXPAND/COPY |

VM 的唯一 unsafe 不变量:**同一 op 的输出区间与任何输入区间不重叠**
(编译器活跃区间打包保证;debug 构建逐 op 校验 `validate_layout`)。

## 4. 验证矩阵(在 libmicyou 的 CI `native-infer.yml` 执行)

| 层 | 对拍双方 | 工具(libmicyou) | 容差/结果 |
|---|---|---|---|
| ① 算子语义 | numpy 解释器 ↔ onnxruntime | `parity_ort.py --frames 32` | 流式 32 帧全输出,max_abs ≤ ~2e-5 |
| ①′ 逐层 | 同上,所有 1105/1581 个中间张量 | `parity_ort.py --all-nodes` | 层级别定位任何语义偏差 |
| ② 编译忠实性 | MCYI blob(numpy 重放)↔ 解释器 | `replay_check.py --frames 16` | 位精确(绝大多数 0.0) |
| ③ blob 新鲜度 | 重编译 ↔ 已提交产物 | `compile_models.py --check` | 字节级一致(格式确定性) |
| ④ Rust 内核 | VM ↔ numpy golden(182 个单算子用例) | `cargo test -p micyou-infer --test ops` | abs ≤ 2e-4 |
| ⑤ Rust 端到端 | VM 流式自反馈 16 帧 ↔ 解释器轨迹 | `cargo test -p micyou-infer --test e2e` | 实测 worst rel 4.8e-6 (pv) / 1.2e-5 (aec) |
| ⑥ DSP 布线 | 处理器时域输出 ↔ numpy 时域复刻 | `cargo test -p micyou-audio`(时域 golden) | STFT/cache/OLA 接线 golden |
| ⑦ 性能 | native VM ↔ ort(main 基线) | `bench` job(同一 runner 顺序执行) | 见 §5 |

①–⑦ 在 libmicyou 的移植分支上全部通过(该仓库的 VM 源码、blob 与本仓库
逐字节一致;本仓库分支 `feat/pure-rust-inference-full` 曾携带完整工具链与
fixtures 复跑过同一矩阵,CI 全绿,可作对拍记录)。本仓库内保留的是
**零夹具行为测试**:`micyou-audio` dsp.rs `native_tests`(PureVox 衰减稳态
噪声、AEC7 压制对齐噪声回声、全链 smoke),`cargo test -p micyou-audio
--features noise-suppression` 即可运行,无需任何外部文件。

## 5. 性能(与 ort 版对比)

性能对比在 libmicyou 的移植 CI 中完成:同一 GitHub runner
(ubuntu-22.04,单线程)上分别对当时的 `main`(ort 2.0.0-rc.13 +
onnxruntime 1.19.2,`intra/inter_threads=1`)与移植分支(纯 Rust VM)运行
同一 harness(`xtask/bench-{ort,native}`,留在 libmicyou 仓库;相同确定性
信号、300 warmup + 3000 计量帧、480 样本/帧 = 10 ms 实时预算)。
两侧 DSP 布线一致,结论直接适用于本仓库。

**实测(libmicyou run 36326547644,2026-09-27,每帧延迟):**

| 配置 | ort mean | native mean | native/ort | native p95 | native RTF |
|---|---:|---:|---:|---:|---:|
| 基线链(无模型) | 5 µs | 5 µs | 1.00× | 5 µs | 0.0005 |
| purevox6 | 1002 µs | **985 µs** | **0.98×** | 1000 µs | 0.098 |
| aec7 | 2380 µs | 3062 µs | 1.29× | 3155 µs | 0.306 |
| **aec7 + purevox6** | 3712 µs | **3844 µs** | **1.04×** | 3943 µs | **0.384** |

- 组合生产配置下与 ort **基本打平(1.04×)**,RTF 0.384 → 实时余量 ~2.6×;
  purevox6 单模型已反超 MLAS。
- 首版朴素 VM 为 2.7–3.5×(aec7 7.8 ms,RTF 1.06 超预算);一轮剖析驱动优化
  (AVX2+FMA 运行时派发、无分支 exp/tanh、GRU 权重 L1 复用与预转置、
  reduce 向量化特例、DW conv axpy 化)后到达上表数字。
- aec7 剩余 ~29% 差距来自 MLAS 的 AVX GEMM 微内核 vs 本 VM 的直白循环,
  以及每帧 ~1.2 MB 的缓存回拷;后续可做 GEMM 分块、ConvT 重排与
  缓存 ping-pong(见 §7)。
- GitHub runner 为共享虚机,绝对值有噪声(~5%),比值取同 run 内对比。

## 6. 目录导航

```
libmicyou 仓库(模型生产与验证):
  models(权重来源)                      # {purevox6,aec7_ep0185}.onnx
  tools/onnx-port/
    onnxref/interpreter.py              # 通用 numpy ONNX 解释器(语义蓝本)
    onnxref/compiler.py                 # ONNX → MCYI 静态编译器
    onnxref/replay.py                   # MCYI blob 的 numpy 重放器(= VM 可执行规格)
    parity_ort.py / replay_check.py     # ①①′② 对拍
    compile_models.py                   # ③ 编译/新鲜度校验 CLI
    gen_golden.py / gen_td_golden.py    # ④⑤⑥ golden fixtures 生成
  crates/micyou-infer/tests/            # ④⑤ golden 测试与 fixtures
  xtask/bench-{native,ort}/             # ⑦ 性能对比 harness(独立 workspace)

本仓库(运行时):
  tauri-app/crates/micyou-infer/        # 纯 Rust VM(零依赖)
  tauri-app/crates/micyou-audio/src/assets/*.mcy   # 编译好的权重 blob
  tauri-app/crates/micyou-audio/src/dsp.rs         # 处理器接线 + 行为测试
```

## 7. 已知边界与后续优化空间

- VM 只支持这两个模型编译出的算子子集与静态 shape(编译器对其他图会显式报错,
  而非静默降级);
- Resize 仅 linear+half_pixel+sizes 驱动;GRU 仅默认激活(Sigmoid/Tanh);
- 若上游更换模型(如 PureVox 202609 三件套契约):在 libmicyou 重跑
  `compile_models.py` 与 golden 生成、通过其 CI 全链验证后,把新的 `.mcy`
  提交回本仓库(新算子需求会在编译期显式暴露,需同步补
  `interpreter.py`/`compiler.py`/`replay.py`/Rust 内核四处);
- f32 求和序与 ORT 存在 ~1e-6 级差异(与 silero 移植同量级),听感无影响,
  流式反馈下不发散(16 帧自反馈漂移实测 ≤1.4e-5,系统收缩性良好);
- 性能后续空间(按剖析占比排序):pointwise conv 的 mi 方向寄存器分块、
  ConvTranspose scatter 重排、AEC 缓存 ping-pong 消除每帧 ~1.2 MB 回拷、
  SiLU(sigmoid×mul)算子融合;AVX-512 派发档亦可直接挂进现有
  `have_avx2` 同款机制。
