use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    Chinese,
}

impl Language {
    pub fn detect() -> Self {
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
            .unwrap_or_default();
        if locale.to_lowercase().starts_with("zh") {
            Self::Chinese
        } else {
            Self::English
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::English => Self::Chinese,
            Self::Chinese => Self::English,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Chinese => "简体中文",
        }
    }
    pub fn message(self, text: &str) -> String {
        let translated = self.tr(text);
        if self == Self::English || translated.as_ref() != text {
            return translated.into_owned();
        }
        if let Some(name) = text
            .strip_prefix("Queued ")
            .and_then(|v| v.strip_suffix(" · press 2 to view transfers"))
        {
            return format!("已加入队列：{name} · 按 2 查看传输");
        }
        if let Some(name) = text
            .strip_prefix("Library ")
            .and_then(|v| v.strip_suffix(" is ready"))
        {
            return format!("资料库 {name} 已就绪");
        }
        for (en, zh) in [
            ("Status: ", "状态："),
            ("Path: ", "路径："),
            ("Size: ", "大小："),
            ("From: ", "来源："),
            ("To: ", "目标："),
            ("Account information unavailable: ", "无法读取账号信息："),
        ] {
            if let Some(rest) = text.strip_prefix(en) {
                return format!("{zh}{}", self.tr(rest));
            }
        }
        if let Some((name, status)) = text.rsplit_once(" · ") {
            if ["complete", "failed", "cancelled"].contains(&status) {
                return format!("{name} · {}", self.tr(status));
            }
        }
        text.to_string()
    }
    pub fn format(self, template: &str, values: &[String]) -> String {
        let mut value = self.tr(template).into_owned();
        for (index, argument) in values.iter().enumerate() {
            value = value.replace(&format!("{{{index}}}"), argument);
        }
        value
    }
    pub fn tr<'a>(self, key: &'a str) -> Cow<'a, str> {
        if self == Self::English {
            return Cow::Borrowed(key);
        }
        let trimmed = key.trim();
        let Some((_, translated)) = TRANSLATIONS
            .iter()
            .find(|(en, _)| en.trim().eq_ignore_ascii_case(trimmed))
        else {
            return Cow::Borrowed(key);
        };
        if key == trimmed {
            Cow::Borrowed(translated)
        } else {
            let start = key.len() - key.trim_start().len();
            let end = key.trim_end().len();
            Cow::Owned(format!("{}{}{}", &key[..start], translated, &key[end..]))
        }
    }
}

const TRANSLATIONS: &[(&str, &str)] = &[
    ("DOWNLOAD · CHOOSE A DIRECTORY", "下载 · 选择保存目录"),
    (
        "Type a directory path. Tab completes; Enter opens or selects.",
        "输入目录路径，Tab 补全，Enter 进入或选用。",
    ),
    ("Use this directory", "使用此目录"),
    ("{0} directories", "{0} 个目录"),
    (
        "↑↓ select  tab complete  enter open/use  esc cancel",
        "↑↓ 选择  tab 补全  enter 进入/选用  esc 取消",
    ),
    (
        "Ctrl+D use directory · Alt+↑ parent · Alt+. hidden · Ctrl+L clear",
        "Ctrl+D 选用 · Alt+↑ 上级 · Alt+. 隐藏项 · Ctrl+L 清空",
    ),
    (
        "Choose an existing local directory.",
        "请选择已有的本地目录。",
    ),
    ("Loading libraries", "正在加载资料库"),
    ("Revoking share link", "正在撤销分享链接"),
    ("Token rejected", "Token 验证失败"),
    ("Remote directory", "远程目录"),
    ("{0} files selected", "已选择 {0} 个文件"),
    ("· first 2000 matches", "· 仅显示前 2000 个匹配"),
    (
        "Ctrl+L clear path · Alt+↑ parent · Alt+. hidden · Ctrl+Space select",
        "Ctrl+L 清空 · Alt+↑ 上级 · Alt+. 隐藏项 · Ctrl+Space 多选",
    ),
    (
        "Recursive patterns are not supported. Choose a directory and a file-name pattern.",
        "暂不支持递归通配符，请选择目录后匹配文件名。",
    ),
    ("FILES", "文件"),
    ("Transfers", "传输"),
    ("Shares", "分享"),
    ("SHARE LINKS", "分享链接"),
    ("Settings", "设置"),
    ("Language", "界面语言"),
    ("Account", "账号"),
    ("Switch account", "切换账号"),
    ("Sign in with token", "使用 Token 登录"),
    ("Add account", "添加账号"),
    ("Sign out", "退出登录"),
    ("Not signed in", "未登录"),
    ("Environment account", "环境变量账号"),
    ("CLI account", "CLI 配置账号"),
    ("Settings saved", "设置已保存"),
    ("Account switched", "已切换账号"),
    ("Signed out", "已退出登录"),
    ("Signing in", "正在登录"),
    ("Sign in", "登录"),
    ("Token", "Token"),
    ("Account name (optional)", "账号名称（可选）"),
    ("Token cannot be empty.", "Token 不能为空。"),
    (
        "Paste the token only, without spaces or line breaks.",
        "请只粘贴 Token，不要包含空格或换行。",
    ),
    (
        "Finish or cancel your transfers before switching accounts.",
        "请先完成或取消传输，再切换账号。",
    ),
    (
        "Sign in to browse your cloud drive.",
        "登录后即可浏览云盘。",
    ),
    ("Saved accounts", "已保存的账号"),
    ("No saved accounts", "暂无已保存的账号"),
    ("Account settings", "账号设置"),
    ("Language and account settings", "语言与账号设置"),
    ("Accounts", "账号"),
    ("Switch between saved accounts", "在已保存的账号之间切换"),
    ("Add an account using a token", "使用 Token 添加账号"),
    ("Sign out of this workspace", "退出当前工作区"),
    ("cloud workspace", "云盘工作区"),
    ("  /  cloud workspace", "  /  云盘工作区"),
    ("DEMO · offline", "演示 · 离线"),
    ("● connected", "● 已连接"),
    ("◌ connecting", "◌ 连接中"),
    ("○ disconnected", "○ 未连接"),
    ("LIBRARIES", "资料库"),
    ("NAME", "名称"),
    ("SIZE", "大小"),
    ("INSPECTOR", "详情"),
    ("LOCATION", "位置"),
    ("ACTIONS", "操作"),
    ("FOLDER", "文件夹"),
    ("FILE", "文件"),
    ("FILE / LINK", "文件 / 链接"),
    ("LIBRARY", "资料库"),
    ("VIEWS", "访问"),
    ("EXPIRES", "到期时间"),
    ("No expiry", "永久"),
    ("No libraries yet", "暂无资料库"),
    ("New library", "新建资料库"),
    ("Your files, in the terminal.", "在终端管理你的文件。"),
    ("Loading your files", "正在加载文件"),
    (
        "You can keep using commands while this loads.",
        "加载时仍可使用快捷键。",
    ),
    ("Could not load this directory", "目录加载失败"),
    ("Your cloud workspace", "你的云盘工作区"),
    (
        "Tab to choose a library, or N to create one.",
        "按 Tab 选择资料库，按 N 新建。",
    ),
    ("Ctrl+P opens commands.", "按 Ctrl+P 打开命令面板。"),
    ("4  Settings    Ctrl+P  Commands", "4  设置    Ctrl+P  命令"),
    ("A little room for what's next", "目录为空"),
    ("No matching files", "没有匹配的文件"),
    (
        "u  Upload a file    n  New folder",
        "u  上传文件    n  新建文件夹",
    ),
    (
        "Change your filter, or press Esc to clear it.",
        "更改筛选条件，或按 Esc 清除。",
    ),
    (
        "Space select files · a select all",
        "Space 选择文件 · a 全选",
    ),
    ("Select a file to see its details.", "选择文件以查看详情。"),
    ("Open folder", "打开文件夹"),
    ("Download", "下载"),
    ("Create share link", "创建分享链接"),
    ("Copy remote path", "复制远程路径"),
    ("A clear runway", "暂无传输任务"),
    (
        "Upload or download a file from the Files workspace.",
        "在文件工作区上传或下载文件。",
    ),
    (
        "Transfers keep moving while you browse.",
        "浏览目录时传输会继续。",
    ),
    ("Shared from your drive", "来自你的云盘"),
    ("Loading share links", "正在加载分享链接"),
    ("Could not load share links", "分享列表加载失败"),
    ("Good work is worth sharing", "暂无分享链接"),
    ("No matching links", "没有匹配的链接"),
    (
        "Select a file or folder and press s to create a link.",
        "选择文件或文件夹后按 s 创建分享。",
    ),
    ("Shared item", "分享项目"),
    (
        "Cancelling active transfers before exit…",
        "正在取消传输，完成后退出……",
    ),
    (
        "Offline demo · cloud operations are disabled",
        "离线演示 · 云盘操作已停用",
    ),
    ("Local", "本地"),
    ("COMMANDS", "命令面板"),
    ("Find", "查找"),
    ("Find an action", "搜索操作"),
    ("No matching commands", "没有匹配的命令"),
    ("WORKING", "处理中"),
    ("Waiting for the cloud drive…", "正在等待云盘响应……"),
    ("SHARE LINK READY", "分享链接已创建"),
    ("↗  Ready to share", "↗  可以分享了"),
    ("Password protected", "已设置密码"),
    ("No password", "无密码"),
    ("Copy link to clipboard", "复制链接"),
    ("Back to files", "返回文件"),
    ("REVOKE SHARE LINK", "撤销分享链接"),
    ("Revoke this link?", "撤销此分享链接？"),
    (
        "People using this link will lose access. Your file stays in the drive.",
        "此链接将停止访问，云盘中的文件仍会保留。",
    ),
    ("Revoke link", "撤销链接"),
    ("Keep link", "保留链接"),
    ("TRANSFERS ARE STILL RUNNING", "传输仍在进行"),
    (
        "Quit and cancel the remaining transfers?",
        "取消剩余传输并退出？",
    ),
    (
        "Partial downloads can resume when retried.",
        "重试时会尝试续传已下载的部分。",
    ),
    ("Cancel transfers and quit", "取消传输并退出"),
    ("Keep working", "继续使用"),
    ("Details", "详情"),
    ("esc / enter  Close", "esc / enter  关闭"),
    ("MAKE YOURSELF AT HOME", "快捷键帮助"),
    ("Your cloud drive. One keyboard.", "使用键盘管理你的云盘。"),
    (
        "Files / Transfers / Shares / Settings",
        "文件 / 传输 / 分享 / 设置",
    ),
    ("Search commands; Tab completes", "搜索命令，Tab 补全"),
    ("Move selection; PgUp/PgDn page", "移动选择，PgUp/PgDn 翻页"),
    (
        "Open library, folder, or details",
        "打开资料库、文件夹或详情",
    ),
    (
        "Parent folder; at root: libraries",
        "返回上层；根目录返回资料库",
    ),
    ("Switch between libraries and files", "切换资料库和文件焦点"),
    (
        "Fuzzy filter; Enter keeps, Esc clears",
        "模糊筛选，Enter 保留，Esc 清除",
    ),
    (
        "Select file / toggle all visible files",
        "选择文件 / 全选可见文件",
    ),
    (
        "Upload / download / create share link",
        "上传 / 下载 / 创建分享",
    ),
    (
        "New folder / new library / go to path",
        "新建文件夹 / 资料库 / 跳转",
    ),
    (
        "Sort / toggle inspector / copy path",
        "排序 / 详情栏 / 复制路径",
    ),
    ("Refresh; in Transfers: retry", "刷新；传输工作区中重试"),
    (
        "Cancel transfer / confirm share revoke",
        "取消传输 / 确认撤销分享",
    ),
    ("Clear completed transfers", "清除已结束的传输"),
    (
        "Cycle conflict policy in transfer forms",
        "切换传输冲突策略",
    ),
    ("Close overlay / quit workspace", "关闭弹窗 / 退出"),
    (
        "Default conflict policy: uniquify. Folder transfers are not supported.",
        "默认冲突策略：自动改名。暂不支持文件夹传输。",
    ),
    (
        "Mouse: click to select, double-click to open, wheel to scroll.",
        "鼠标：单击选择，双击打开，滚轮移动列表。",
    ),
    ("Upload a file", "上传文件"),
    ("Upload files", "上传文件"),
    ("Download files", "下载文件"),
    ("Create a share link", "创建分享链接"),
    ("Go to a directory", "跳转目录"),
    ("New folder", "新建文件夹"),
    ("Local file", "本地文件"),
    ("Remote destination", "远程目标"),
    (
        "Save into an existing local directory",
        "保存到已有的本地目录",
    ),
    ("Password (optional)", "密码（可选）"),
    (
        "Expires in days (blank: no expiry)",
        "有效天数（留空为永久）",
    ),
    ("Library or remote path", "资料库或远程路径"),
    ("Library name", "资料库名称"),
    ("Folder name", "文件夹名称"),
    (
        "Choose the destination and conflict policy before uploading.",
        "确认上传目标和冲突策略。",
    ),
    (
        "Library name or repo:<library>/<directory>",
        "资料库名称，或 repo:<资料库>/<目录>",
    ),
    (
        "Creates a folder inside the current directory.",
        "在当前目录内创建文件夹。",
    ),
    (
        "A new home for a collection of files.",
        "创建用于存放文件的资料库。",
    ),
    (
        "Paste a cloud drive API token. It will be saved privately on this device.",
        "粘贴云盘 API Token。验证后会私密保存在本机。",
    ),
    ("UPLOAD · CHOOSE A LOCAL FILE", "上传 · 选择本地文件"),
    (
        "Folders open with Enter. Select a file to review its upload destination.",
        "Enter 打开目录；选择文件后确认上传位置。",
    ),
    (
        "No files in this folder. Press . to show hidden files.",
        "目录中没有文件，按 . 显示隐藏文件。",
    ),
    ("shown", "显示"),
    ("hidden", "隐藏"),
    ("Browse your cloud libraries", "浏览云盘资料库"),
    ("Choose a local file to upload", "选择要上传的本地文件"),
    ("Download the selected files", "下载选中的文件"),
    (
        "Create a link with optional password and expiry",
        "创建分享，可设置密码和有效期",
    ),
    (
        "Track, cancel, or retry your transfers",
        "查看、取消或重试传输",
    ),
    (
        "Browse and revoke existing share links",
        "查看或撤销已有的分享链接",
    ),
    (
        "Jump to a library or remote directory",
        "跳转到资料库或远程目录",
    ),
    (
        "Create a folder in the current library",
        "在当前资料库中新建文件夹",
    ),
    ("Create a new library", "创建新的资料库"),
    ("Reload the current workspace", "刷新当前工作区"),
    (
        "Cycle file sorting: name, reverse name, size",
        "切换名称正序、倒序和大小排序",
    ),
    ("Toggle the file inspector", "显示或收起详情栏"),
    (
        "Keyboard shortcuts and workflow guide",
        "查看快捷键和操作帮助",
    ),
    ("Leave the workspace", "退出工作区"),
    ("Upload", "上传"),
    ("Share", "分享"),
    ("Goto", "跳转"),
    ("Mkdir", "新建文件夹"),
    ("Mkrepo", "新建资料库"),
    ("Refresh", "刷新"),
    ("Sort", "排序"),
    ("Help", "帮助"),
    ("Quit", "退出"),
    ("name ↑", "名称 ↑"),
    ("name ↓", "名称 ↓"),
    ("size ↓", "大小 ↓"),
    ("queued", "等待中"),
    ("transferring", "传输中"),
    ("complete", "已完成"),
    ("failed", "失败"),
    ("cancelled", "已取消"),
    ("↑ upload", "↑ 上传"),
    ("↓ download", "↓ 下载"),
    (
        "↑↓ choose   enter switch   esc close",
        "↑↓ 选择   enter 切换   esc 关闭",
    ),
    (
        "↑↓ choose   enter run   tab complete   esc close",
        "↑↓ 选择   enter 执行   tab 补全   esc 关闭",
    ),
    ("↑↓ scroll   esc close", "↑↓ 滚动   esc 关闭"),
    (
        "tab next field   enter confirm   esc cancel",
        "tab 下一项   enter 确认   esc 取消",
    ),
    (
        "↑↓ navigate   enter change   ←→ language   1 files   esc back   q quit",
        "↑↓ 选择   enter 更改   ←→ 语言   1 文件   esc 返回   q 退出",
    ),
    (
        "↑↓ navigate   enter open library   tab files   N new library   4 settings   ? help   q quit",
        "↑↓ 选择   enter 打开资料库   tab 文件   N 新建   4 设置   ? 帮助",
    ),
    (
        "↑↓ navigate   enter open   tab libraries   u upload   d download   s share   f find   4 settings   ? help",
        "↑↓ 选择   enter 打开   tab 资料库   u 上传   d 下载   s 分享   f 查找   4 设置   ? 帮助",
    ),
    (
        "↑↓ move  enter open  tab libraries  u upload  4 settings  ? help",
        "↑↓ 选择  enter 打开  tab 资料库  u 上传  4 设置  ? 帮助",
    ),
    (
        "↑↓ navigate   x cancel   r retry   enter details   1 files   ? help   q quit",
        "↑↓ 选择   x 取消   r 重试   enter 详情   1 文件   ? 帮助   q 退出",
    ),
    (
        "↑↓ navigate   y copy   x revoke   f find   r refresh   1 files   ? help   q quit",
        "↑↓ 选择   y 复制   x 撤销   f 查找   r 刷新   1 文件   ? 帮助",
    ),
    (
        "x cancel  ·  r retry failed/cancelled  ·  enter details  ·  C clear finished",
        "x 取消 · r 重试失败/取消任务 · enter 详情 · C 清除已结束任务",
    ),
    (
        "y copy link  ·  enter details  ·  x revoke  ·  f filter  ·  r refresh",
        "y 复制链接 · enter 详情 · x 撤销 · f 筛选 · r 刷新",
    ),
    (
        "This workspace needs at least 60 × 18 cells.",
        "终端尺寸至少需要 60 × 18。",
    ),
    (
        "Resize your terminal, or press q to quit.",
        "请调整终端尺寸，或按 q 退出。",
    ),
    ("Conflict", "重名时"),
    ("uniquify", "自动改名"),
    ("overwrite", "覆盖"),
    ("fail", "报错"),
    ("Ctrl O changes", "Ctrl O 更改"),
    ("Browse", "浏览"),
    ("Path", "路径"),
    ("Matching paths", "匹配路径"),
    ("No matching paths", "没有匹配的路径"),
    (
        "Type a path, file name, or pattern. Tab completes; Enter opens or selects.",
        "输入路径、文件名或通配符。Tab 补全，Enter 打开或选择。",
    ),
    (
        "↑↓ select  tab complete  enter choose  ctrl+a all matches  esc cancel",
        "↑↓ 选择  tab 补全  enter 确认  ctrl+a 全部匹配  esc 取消",
    ),
    ("Get token", "获取 Token"),
    (
        "Open a library first. Tab switches to the library list.",
        "请先打开资料库。按 Tab 切换到资料库列表。",
    ),
    (
        "Select a file to download. Space selects multiple files.",
        "请选择要下载的文件，Space 可多选。",
    ),
    (
        "Select a file or folder to share.",
        "请选择要分享的文件或文件夹。",
    ),
    ("Copied to clipboard", "已复制到剪贴板"),
    ("Folder created", "文件夹已创建"),
    ("Share link revoked", "分享链接已撤销"),
    (
        "The download directory must already exist.",
        "下载目录必须已存在。",
    ),
    (
        "Expiry must be a positive number of days.",
        "有效期必须是正整数。",
    ),
    (
        "Choose an existing local file. Directories are not supported.",
        "请选择已有的本地文件，暂不支持上传文件夹。",
    ),
    (
        "Use a relative folder name without empty, '.' or '..' segments.",
        "请输入相对目录名，不要包含空段、. 或 ..。",
    ),
    (
        "Library names cannot contain '/'.",
        "资料库名称不能包含 /。",
    ),
    ("Transfer complete", "传输完成"),
    ("Cancelled before starting", "开始前已取消"),
    ("Cancelling…", "正在取消……"),
    (
        "Cancelled. Partial downloads can resume on retry.",
        "已取消，重试时会尝试续传。",
    ),
    (
        "Saved with a unique name to avoid overwriting",
        "已自动改名保存，避免覆盖",
    ),
    (
        "Finishing the server request. You can quit when it completes.",
        "正在完成云盘请求，完成后可退出。",
    ),
    (
        "This demo is read-only. Run thufs tui to work with your cloud drive.",
        "这是只读演示，请运行 thufs tui 操作真实云盘。",
    ),
    ("This demo is read-only.", "这是只读演示。"),
    ("{0} items", "{0} 个项目"),
    (
        "{0} of {1} items · filter: {2}",
        "{0} / {1} 个项目 · 筛选：{2}",
    ),
    (
        "{0} selected · d download · Esc clear",
        "已选择 {0} 个 · d 下载 · Esc 清除",
    ),
    ("{0} active", "{0} 个进行中"),
    ("{0} completed", "{0} 个已完成"),
    ("{0} total", "共 {0} 个"),
    ("{0} links", "{0} 个链接"),
    ("{0} active transfer(s)", "{0} 个传输正在进行"),
    (
        "{0} file(s) will be added to your transfer queue.",
        "将把 {0} 个文件加入传输队列。",
    ),
    ("Expires in {0} days", "{0} 天后到期"),
    ("Loading local files", "正在加载本地文件"),
];
