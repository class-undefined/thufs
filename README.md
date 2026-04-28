# thufs

[![GitHub stars](https://img.shields.io/github/stars/class-undefined/thufs?style=flat-square)](https://github.com/class-undefined/thufs/stargazers)
[![GitHub license](https://img.shields.io/github/license/class-undefined/thufs?style=flat-square)](https://github.com/class-undefined/thufs/blob/master/LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/class-undefined/thufs/ci.yml?branch=master&style=flat-square&label=CI)](https://github.com/class-undefined/thufs/actions/workflows/ci.yml)
[![Publish crate](https://img.shields.io/github/actions/workflow/status/class-undefined/thufs/publish-crate.yml?branch=master&style=flat-square&label=crate)](https://github.com/class-undefined/thufs/actions/workflows/publish-crate.yml)
[![Crates.io](https://img.shields.io/crates/v/thufs?style=flat-square)](https://crates.io/crates/thufs)
[![GitHub last commit](https://img.shields.io/github/last-commit/class-undefined/thufs?style=flat-square)](https://github.com/class-undefined/thufs/commits/master)
[![GitHub issues](https://img.shields.io/github/issues/class-undefined/thufs?style=flat-square)](https://github.com/class-undefined/thufs/issues)
[![Rust](https://img.shields.io/badge/Rust-2024%20Edition-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Backend: Seafile](https://img.shields.io/badge/Backend-Seafile-ff6b6b?style=flat-square)](https://www.seafile.com/)
[![README in English](https://img.shields.io/badge/README-English-blue?style=flat-square)](./README.en.md)

`thufs` 是一个面向清华云盘终端用户的 CLI 工具，基于 Seafile API 实现，强调“上传、下载、列目录、分享”这类高频、可脚本化的文件操作。

它不试图成为一个重量级同步客户端，而是希望在 shell、远程服务器、集群作业、自动化脚本这些场景里，提供足够直接、稳定、可预期的命令行体验。

## 项目目标

- 面向清华云盘日常用户，尤其是依赖终端和脚本的人群
- 聚焦单文件上传、下载、列目录、分享链接等核心能力
- 保持 Unix 风格的扁平命令接口，便于组合和自动化
- 默认提供人类可读输出，同时支持 `--json` 供脚本消费

## 功能特性

- `info` 查看当前 token 对应的账号信息
- `repos` 或 `libraries` 查看当前可见的仓库或资料库
- `ls` 查看远程目录内容，支持 repo 根目录
- `upload` 上传本地文件到清华云盘
- `download` 从清华云盘下载文件到本地
- `share` 生成分享链接，可选密码和过期时间
- `mkrepo` 或 `mklib` 主动创建 library
- `mkdir` 主动创建远程目录，并自动创建父目录
- `push` 和 `pull` 作为兼容别名保留
- 上传和下载在 TTY 下显示进度条
- 上传和下载支持 `--progress jsonl`，向 stderr 输出机器可读的流式进度事件
- 上传和下载尽力支持断点续传
- 冲突策略统一为 `--conflict`
- 默认冲突策略为 `uniquify`，避免静默覆盖
- JSON 输出中返回最终落点路径，便于脚本追踪自动重命名结果
- `upload` 在目标 library 或目录不存在时会自动创建
- `ls` 支持展示更新时间

## 为什么是 thufs

清华云盘的官方使用方式更偏向 Web 或桌面同步，而很多终端用户真正需要的是：

- 在服务器上把实验结果直接上传到云盘
- 在脚本里拉取某个共享文件作为输入
- 在 cron、集群作业、自动化脚本里稳定地保存产物
- 快速查看某个库里的内容，而不是启动图形界面

`thufs` 针对的就是这类需求。它更像 `scp`、`rsync`、`curl` 这种“拿来即用”的命令工具，而不是一个常驻同步守护进程。

## 安装

直接通过 Cargo 安装：

```bash
cargo install thufs --locked
thufs --help
```

当前最低支持 Rust 版本为 `1.85`。如果直接 `cargo install thufs` 遇到依赖解析到更高 Rust 版本的问题，请使用 `--locked`。

也可以从源码构建：

```bash
git clone git@github.com:class-undefined/thufs.git
cd thufs
cargo build --release
./target/release/thufs --help
```

macOS 用户也可以直接下载 GitHub Release 中对应架构的预编译二进制包。

如果你的网络环境访问 Rust 官方源较慢，可使用国内镜像：

```bash
export RUSTUP_DIST_SERVER="https://rsproxy.cn"
export RUSTUP_UPDATE_ROOT="https://rsproxy.cn/rustup"
```

如果仅需本地开发调试：

```bash
cargo run -- --help
```

## 认证与配置

`thufs` 当前采用 token 驱动，不实现用户名密码登录，也不处理浏览器 OAuth 流程。你需要先从外部获取可用的 Seafile API token。

获取 token 的推荐方式：

1. 使用清华账号登录清华云盘页面：

```text
https://cloud.tsinghua.edu.cn/profile/#get-auth-token
```

2. 在页面中的 `Web API Auth Token` 区域点击“生成链接”
3. 获取页面展示的 token 信息，并将其用于 `thufs auth set-token`

设置 token：

```bash
thufs auth set-token <seafile-api-token>
```

查看当前配置：

```bash
thufs config show
thufs --json config show
```

默认配置文件路径：

```text
~/.config/thufs/config.json
```

支持的环境变量覆盖：

- `THUFS_TOKEN`
- `THUFS_DEFAULT_REPO`
- `THUFS_OUTPUT`
- `THUFS_CONFIG_DIR`

设计原则是“配置文件优先，环境变量覆盖”，这样既适合日常使用，也适合临时脚本注入。

## 远程路径规则

显式远程路径的标准形式为：

```text
repo:<library>/<path>
```

这里的 `library` 就是你在 Seafile 或清华云盘中可见的仓库名、资料库名。

例如：

```text
repo:course-lib/slides/week1.pdf
```

表示：

```text
仓库 course-lib 中的 /slides/week1.pdf
```

### 合法示例

```bash
thufs ls repo:course-lib/slides
thufs download repo:course-lib/slides/week1.pdf
thufs upload ./report.pdf repo:course-lib/submissions/report.pdf
thufs upload ./report.pdf repo:course-lib
```

### 默认仓库简写

如果配置了 `THUFS_DEFAULT_REPO` 或本地配置中的 `default_repo`，则可以使用简写路径：

```bash
thufs ls slides
thufs download slides/week1.pdf
thufs upload ./report.pdf submissions/
```

此时这些路径会被解释为当前默认仓库下的路径。

### 关于 `repo:<library>` 与 `repo:<path>`

`repo:<library>/<path>` 是有意义的，因为 Seafile 的文件定位天然依赖“仓库 + 仓库内路径”两段信息。

`repo:<library>` 在 `upload` 场景下也被支持，用来表示“上传到这个仓库根目录，并沿用本地文件名”。

而单独的 `repo:<path>` 在没有仓库名时是不完整的，因此不作为标准语法。

## 快速开始

### 查看账号信息

```bash
thufs info
thufs --json info
```

### 查看可见仓库

```bash
thufs repos
thufs libraries
thufs --json repos
```

### 查看目录

```bash
thufs ls repo:course-lib/slides
thufs ls course-lib
thufs --json ls repo:course-lib/slides
```

`ls` 的文件体积会自适应显示为 `B`、`KB`、`MB`、`GB` 等单位。
如果需要更新时间信息，可加 `--time` 或 `-t`。

```bash
thufs ls -t repo:course-lib/slides
thufs ls --time course-lib
```

### 上传文件

```bash
thufs upload ./report.pdf repo:course-lib/submissions/report.pdf
thufs upload ./report.pdf repo:course-lib
thufs upload ./report.pdf submissions/
```

说明：

- 如果目标写成 `repo:course-lib`，默认上传到仓库根目录，并使用本地文件名
- 如果目标以 `/` 结尾且已配置默认仓库，如 `submissions/`，也会默认使用本地文件名
- 如果目标 library 不存在，`upload` 会自动创建 library
- 如果目标父目录不存在，`upload` 会自动创建远程目录
- `push` 是 `upload` 的兼容别名

### 主动创建 library 或目录

```bash
thufs mkrepo course-lib
thufs mklib course-lib
thufs mkdir repo:course-lib/slides/week1
thufs mkdir submissions/week1
```

### 下载文件

```bash
thufs download repo:course-lib/slides/week1.pdf
thufs download repo:course-lib/slides/week1.pdf ./week1.pdf
thufs download repo:course-lib/slides/week1.pdf ./downloads/
thufs download https://cloud.tsinghua.edu.cn/f/abc123XYZ_/
thufs download "https://cloud.tsinghua.edu.cn/f/abc123XYZ_/?dl=1"
thufs download --share abc123XYZ_
thufs download --mode sequential repo:course-lib/slides/week1.pdf
thufs download --mode parallel --workers 8 repo:course-lib/slides/week1.pdf
```

说明：

- 如果不写本地路径，默认保存到当前目录，并使用远程文件名
- 如果本地路径是已存在目录，则下载到该目录下并沿用远程文件名
- 也支持分享文件下载：可直接传完整分享链接，`?dl=1` 这类查询参数会被忽略，只提取其中的 token；或使用 `--share <hashcode>`
- 分享下载会根据当前登录状态决定身份：已配置 token 时会携带登录身份，未登录时按匿名下载处理，便于访问需要身份授权的分享链接
- `pull` 是 `download` 的兼容别名

### 创建分享链接

```bash
thufs share repo:course-lib/slides/week1.pdf
thufs share --password secret --expire-days 7 repo:course-lib/slides/week1.pdf
thufs --json share repo:course-lib/slides/week1.pdf
thufs shares repo:course-lib/slides/week1.pdf
thufs shares --page 1 --per-page 50
thufs shares --all
thufs unshare <share-token>
```

## 冲突策略

`upload` 和 `download` 统一支持：

```bash
--conflict <policy>
```

可选策略：

- `uniquify` 自动生成不冲突的新文件名，如 `report-(1).pdf`
- `overwrite` 覆盖已存在目标
- `fail` 遇到冲突立即失败
- `prompt` 显式进入交互式确认模式

默认策略：

```text
uniquify
```

这意味着在未指定策略时，`thufs` 会尽量避免覆盖已有文件，并返回实际使用的最终路径。

示例：

```bash
thufs upload --conflict overwrite ./report.pdf repo:course-lib/submissions/report.pdf
thufs upload --conflict uniquify ./report.pdf repo:course-lib/submissions/report.pdf
thufs download --conflict fail repo:course-lib/slides/week1.pdf ./week1.pdf
```

## JSON 输出与脚本化

默认输出偏向人类阅读；传入 `--json` 后会输出结构化 JSON，更适合脚本处理。

例如：

```bash
thufs --json upload ./report.pdf repo:course-lib/submissions/report.pdf
```

上传或下载结果中会包含类似这些字段：

- `requested_remote_path`
- `final_remote_path`
- `requested_local_path`
- `final_local_path`
- `remote_name`
- `local_name`
- `overwritten`
- `uniquified`

因此脚本可以可靠判断：

- 最终是否发生自动重命名
- 最终保存到哪里
- 最终文件名是什么

示例：

```bash
FINAL_PATH="$(thufs --json download repo:course-lib/slides/week1.pdf | jq -r '.final_local_path')"
echo "saved to: $FINAL_PATH"
```

## 传输体验

- 在 stderr 为 TTY 时显示进度条
- `--progress jsonl` 会向 stderr 持续输出 JSON Lines 进度事件，适合 GUI、任务队列和脚本精确追踪百分比
- `--progress none` 可关闭传输进度输出
- `download` 会使用 `.thufs-part` 临时文件，并在服务器支持时尝试断点续传
- `download` 默认使用单线程下载，避免在服务端 Range 支持不稳定时触发不必要的分片请求
- 显式使用 `--mode auto` 时会优先尝试并发分片下载；若自动模式下的 Range 分片请求失败，会清理临时文件并回退到单线程下载
- 自动回退会按进度模式报告为 warning：TTY 模式显示警告，`--progress jsonl` 输出 `warning` 事件，`--progress none` 保持静默；显式 `--mode parallel` 仍会严格失败
- `upload` 会基于 Seafile 的 uploaded-bytes 机制进行尽力续传

机器可读进度示例：

```bash
thufs upload ./report.pdf repo:course-lib/submissions/report.pdf --progress jsonl
thufs download repo:course-lib/slides/week1.pdf ./week1.pdf --progress jsonl
```

每行都是一个独立 JSON 事件，包含：

- `event`
- `operation`
- `path`
- `transferred_bytes`
- `total_bytes`
- `percent`

下载模式可通过以下参数显式控制：

- `--mode sequential` 默认行为，始终使用单线程下载
- `--mode auto` 优先并发，不满足条件时自动退回单线程
- `--mode parallel` 强制要求并发下载；若服务端不支持则报错
- `--workers N` 指定并发下载的 worker 数，仅对并发下载生效

这类续传能力属于“best effort”，行为仍受服务端支持程度影响。

## 输出约定

- 正常结果输出到 stdout
- 错误输出到 stderr
- `--json` 仅影响正常结果的格式，不改变错误流向

这保证了 `thufs` 更容易被 shell 管道、重定向、`jq`、`xargs` 等工具组合。

## 命令总览

| 命令 | 作用 |
| --- | --- |
| `thufs info` | 查看当前 token 对应账号信息 |
| `thufs repos` | 列出当前可见仓库 |
| `thufs ls <remote>` | 列出远程目录内容 |
| `thufs mkrepo <name>` | 创建 library |
| `thufs mkdir <remote>` | 创建远程目录 |
| `thufs upload <local> <remote>` | 上传本地文件 |
| `thufs download <remote> [local]` | 下载远程文件 |
| `thufs share <remote>` | 创建分享链接 |
| `thufs shares [remote]` | 分页查看分享链接，未指定 remote 时查看全部 |
| `thufs unshare <token>` | 删除分享链接 |
| `thufs auth set-token <token>` | 存储 token |
| `thufs config show` | 查看当前生效配置 |

## 适用场景

- 在 SSH 登录的服务器上直接备份实验结果
- 在集群作业结束后上传输出文件
- 在脚本中获取云盘上的输入数据
- 将云盘分享链接的创建纳入自动流程
- 快速浏览某个仓库下的资料而不打开网页

## 当前边界

`thufs` 当前刻意不覆盖以下方向：

- 完整双向同步
- 递归目录上传或下载
- 多账号配置管理
- 任意 Seafile 实例的通用化接入
- GUI 或桌面常驻同步体验

如果你需要的是“稳定的命令工具”，这些边界是有意保留的；如果你需要“完整同步客户端”，那是另一类产品。

## 开发

运行测试：

```bash
cargo test
```

格式化代码：

```bash
cargo fmt
```

查看帮助：

```bash
cargo run -- --help
cargo run -- upload --help
cargo run -- download --help
```

## 已知说明

- 当前实现基于 Seafile API 形态
- 某些服务端细节可能会因实际部署差异而存在边界行为

## License

本项目采用 [MIT License](./LICENSE)。

## English

English documentation is available at [README.en.md](./README.en.md).
