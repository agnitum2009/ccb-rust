# Root 安装确认设计

## 1. 文档目的

本文档设计 CCB 对 root 安装的兼容策略。

目标不是鼓励 root 使用，而是在确实需要以 root 身份安装和运行的环境中，提供一个明确、可诊断、低误伤的确认流程。

## 2. 背景

当前 `install.sh` 在入口处直接拒绝 root：

```bash
require_non_root_execution
```

这能避免普通用户误用 `sudo` 污染自己的环境，但也导致真实 root 用户无法安装和运行 CCB。

需要区分两类场景：

1. 真实 root 用户把 root 当作独立使用者。
2. 普通用户误用 `sudo ./install.sh install`。

第一类应允许显式确认后继续。第二类应强提醒并默认拒绝。

## 3. 设计目标

必须达成：

1. 非 root 安装行为保持不变。
2. root 安装默认不继续。
3. 交互式 root 安装必须显示强提醒。
4. 用户直接回车或输入非确认值时取消安装。
5. 用户输入 `y` / `Y` / `yes` / `YES` 后允许继续。
6. 非交互 root 安装必须通过显式环境变量确认。
7. doctor 和安装输出应能看出这是 root profile。

## 4. 非目标

第一阶段不做：

- 不自动把 root 安装内容同步给普通用户。
- 不自动 `chown` 普通用户目录。
- 不把 root 的 provider 凭据迁移到普通用户。
- 不修改系统级 `/usr/local/bin`。
- 不默认支持 `sudo` 给普通用户安装。

## 5. 行为规则

### 5.1 非 root

保持现状：

```text
EUID != 0 -> 直接继续安装
```

### 5.2 交互式 root

当满足：

```text
EUID == 0
stdin 是 TTY
CCB_ALLOW_ROOT_INSTALL != 1
```

安装脚本打印强提醒，并询问：

```text
WARN: Root install is not recommended.

You are installing CCB as root.

This will install and run CCB in root's own profile:
  install prefix : /root/.local/share/codex-dual
  bin directory  : /root/.local/bin
  role store     : /root/.local/share/ccb/roles
  tool store     : /root/.local/share/ccb/tools
  provider auth  : root-owned provider homes and credentials

Do not use root unless you intentionally run Codex/Claude/Gemini as root.
If this command was started with sudo by mistake, cancel now and rerun as your normal user.

Continue root install? (y/N):
```

默认取消：

```text
回车 -> cancel
n/N/no -> cancel
其他值 -> cancel
y/Y/yes/YES -> continue
```

### 5.3 非交互 root

当满足：

```text
EUID == 0
stdin 不是 TTY
CCB_ALLOW_ROOT_INSTALL != 1
```

必须失败：

```text
ERROR: Root install requires explicit confirmation.
Re-run with CCB_ALLOW_ROOT_INSTALL=1 only if you intentionally want a root-owned CCB install.
```

这避免 CI、curl pipe、sudo 脚本等场景无提示地装入 root profile。

### 5.4 显式环境变量

允许：

```bash
CCB_ALLOW_ROOT_INSTALL=1 ./install.sh install
```

含义：

```text
用户已显式确认要把 CCB 安装到 root profile。
```

即使设置该变量，也应打印简短提醒：

```text
WARN: Continuing root install because CCB_ALLOW_ROOT_INSTALL=1 is set.
```

## 6. sudo 场景

如果：

```text
EUID == 0
SUDO_USER 非空
SUDO_USER != root
```

应额外提示：

```text
Detected sudo user: <name>
This will not install CCB for <name>; it will install for root.
```

仍可在交互确认后继续，但默认否。

这样兼容确实想通过 `sudo` 做 root profile 安装的用户，同时阻止最常见的误操作。

## 7. 运行态边界

第一阶段只在安装入口做确认，不在每次 `ccb` 运行时追加交互确认。

原因：

- `ccb doctor`、`ccb kill`、`ccb ps` 等命令可能出现在脚本和恢复流程中。
- 运行时交互确认会让非前台命令变得不可预测。
- root 使用者一旦安装完成，应把 root 视为一个独立 profile。

root 运行态必须遵守：

```text
HOME=/root 时，只使用 root 自己的 provider home、roles、tools 和缓存。
不读取普通用户的全局 provider 凭据。
不自动 chown 普通用户项目目录。
不自动把 root profile 内容同步给普通用户。
```

`ccb doctor` 应承担运行态提醒：

```text
root_runtime: true
install_root_owned: true|false
project_owner: <uid:name>
ccb_dir_owner: <uid:name or missing>
sudo_user: <value or None>
```

如果 root 在普通用户项目内运行，doctor 应提醒：

```text
WARN: Running CCB as root in a non-root-owned project can create root-owned .ccb files.
```

## 8. 推荐实现

替换当前 `require_non_root_execution` 为：

```bash
confirm_root_install_if_needed() {
  if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    return 0
  fi

  if [[ "${CCB_ALLOW_ROOT_INSTALL:-}" == "1" ]]; then
    echo "WARN: Continuing root install because CCB_ALLOW_ROOT_INSTALL=1 is set."
    return 0
  fi

  print_root_install_warning

  if [[ ! -t 0 ]]; then
    echo "ERROR: Root install requires explicit confirmation."
    echo "   Re-run with CCB_ALLOW_ROOT_INSTALL=1 only if this is intentional."
    exit 1
  fi

  local reply
  read -r -p "Continue root install? (y/N): " reply
  case "$reply" in
    y|Y|yes|YES)
      return 0
      ;;
    *)
      echo "Installation cancelled"
      exit 1
      ;;
  esac
}
```

主入口改为：

```bash
confirm_root_install_if_needed
```

不再调用旧的硬拒绝函数。

## 9. 安装元数据

`BUILD_INFO.json` 建议增加：

```json
{
  "install_user_id": 0,
  "install_user_name": "root",
  "root_install": true,
  "sudo_user": "bfly"
}
```

这些字段用于 doctor 输出和问题诊断。

## 10. Doctor 输出

`ccb doctor` 建议增加：

```text
user_id: 0
user_name: root
home: /root
root_runtime: true
install_root_owned: true
sudo_user: <value or None>
```

如果 root 运行但安装不是 root profile，doctor 应提示权限风险。

## 11. 测试计划

安装脚本单元测试应覆盖：

1. 非 root 不提示。
2. root 交互输入空值 -> 取消。
3. root 交互输入 `n` -> 取消。
4. root 交互输入 `y` -> 继续。
5. root 非交互且无 `CCB_ALLOW_ROOT_INSTALL` -> 失败。
6. root 非交互且 `CCB_ALLOW_ROOT_INSTALL=1` -> 继续。
7. `SUDO_USER` 存在时输出 sudo 风险提醒。
8. root runtime doctor 输出 root profile 和项目 ownership 提醒。

实现测试时不要真的切换系统 root。建议把 EUID 检测封装为可注入函数，或在 shell snippet 测试中允许覆盖：

```bash
CCB_TEST_EUID=0
```

生产路径仍使用真实 `EUID`。

## 12. 验收标准

root 用户交互安装：

```bash
./install.sh install
```

期望：

```text
显示 root 强提醒
默认回车取消
输入 y 后继续安装到 /root/.local/...
```

root 用户非交互安装：

```bash
printf '' | ./install.sh install
```

期望失败。

root 用户显式非交互安装：

```bash
CCB_ALLOW_ROOT_INSTALL=1 ./install.sh install
```

期望继续，并打印 root install warning。

root 用户运行：

```bash
ccb doctor
ccb --print-version
```

期望：

```text
doctor 明确显示 root_runtime
doctor 明确显示 root profile 路径
如果项目不是 root-owned，则显示 ownership warning
```

## 13. 风险与缓解

风险：

- root provider 凭据和普通用户凭据分离，用户可能以为登录状态共享。
- root 启动 provider CLI 会创建 root-owned session/cache。
- 使用 sudo 运行项目可能在项目 `.ccb` 下产生 root-owned 文件。

缓解：

- 安装前强提醒。
- 默认否。
- 非交互必须显式环境变量。
- doctor 显示 root profile 状态。
- README 中明确普通用户不要 root 安装。
