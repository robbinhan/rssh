use std::fmt;
use std::fmt::Display;

/// 预定义的颜色常量
pub mod colors {
    use super::Color;

    pub const BLACK: Color = Color::Black;
    pub const RED: Color = Color::Red;
    pub const GREEN: Color = Color::Green;
    pub const YELLOW: Color = Color::Yellow;
    pub const BLUE: Color = Color::Blue;
    pub const MAGENTA: Color = Color::Magenta;
    pub const CYAN: Color = Color::Cyan;
    pub const WHITE: Color = Color::White;
    pub const BRIGHT_BLACK: Color = Color::BrightBlack;
    pub const BRIGHT_RED: Color = Color::BrightRed;
    pub const BRIGHT_GREEN: Color = Color::BrightGreen;
    pub const BRIGHT_YELLOW: Color = Color::BrightYellow;
    pub const BRIGHT_BLUE: Color = Color::BrightBlue;
    pub const BRIGHT_MAGENTA: Color = Color::BrightMagenta;
    pub const BRIGHT_CYAN: Color = Color::BrightCyan;
    pub const BRIGHT_WHITE: Color = Color::BrightWhite;
}

/// 终端颜色
#[derive(Debug, Clone, Copy)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    RGB(u8, u8, u8),
}

/// 终端样式
#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub dim: bool,
    pub reverse: bool,
    pub blink: bool,
    pub rapid_blink: bool,
    pub hidden: bool,
    pub framed: bool,
    pub encircled: bool,
    pub overlined: bool,
    pub superscript: bool,
    pub subscript: bool,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            foreground: None,
            background: None,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            dim: false,
            reverse: false,
            blink: false,
            rapid_blink: false,
            hidden: false,
            framed: false,
            encircled: false,
            overlined: false,
            superscript: false,
            subscript: false,
        }
    }
}

impl Style {
    /// 创建一个新的样式
    pub fn new() -> Self {
        Style::default()
    }

    /// 设置前景色
    pub fn fg(mut self, color: Color) -> Self {
        self.foreground = Some(color);
        self
    }

    /// 设置背景色
    pub fn bg(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    /// 设置粗体
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// 设置斜体
    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// 设置下划线
    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// 设置删除线
    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    /// 设置暗色
    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    /// 设置反色
    pub fn reverse(mut self) -> Self {
        self.reverse = true;
        self
    }

    /// 设置闪烁
    pub fn blink(mut self) -> Self {
        self.blink = true;
        self
    }

    /// 设置快速闪烁
    pub fn rapid_blink(mut self) -> Self {
        self.rapid_blink = true;
        self
    }

    /// 设置隐藏
    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    /// 设置框架
    pub fn framed(mut self) -> Self {
        self.framed = true;
        self
    }

    /// 设置环绕
    pub fn encircled(mut self) -> Self {
        self.encircled = true;
        self
    }

    /// 设置上划线
    pub fn overlined(mut self) -> Self {
        self.overlined = true;
        self
    }

    /// 设置上标
    pub fn superscript(mut self) -> Self {
        self.superscript = true;
        self
    }

    /// 设置下标
    pub fn subscript(mut self) -> Self {
        self.subscript = true;
        self
    }

    /// 生成ANSI转义序列
    pub fn to_ansi(&self) -> String {
        let mut codes = Vec::new();

        // 添加样式代码
        if self.bold { codes.push("1".to_string()); }
        if self.dim { codes.push("2".to_string()); }
        if self.italic { codes.push("3".to_string()); }
        if self.underline { codes.push("4".to_string()); }
        if self.strikethrough { codes.push("9".to_string()); }
        if self.reverse { codes.push("7".to_string()); }

        // 添加前景色代码
        if let Some(fg) = self.foreground {
            match fg {
                Color::Black => codes.push("30".to_string()),
                Color::Red => codes.push("31".to_string()),
                Color::Green => codes.push("32".to_string()),
                Color::Yellow => codes.push("33".to_string()),
                Color::Blue => codes.push("34".to_string()),
                Color::Magenta => codes.push("35".to_string()),
                Color::Cyan => codes.push("36".to_string()),
                Color::White => codes.push("37".to_string()),
                Color::BrightBlack => codes.push("90".to_string()),
                Color::BrightRed => codes.push("91".to_string()),
                Color::BrightGreen => codes.push("92".to_string()),
                Color::BrightYellow => codes.push("93".to_string()),
                Color::BrightBlue => codes.push("94".to_string()),
                Color::BrightMagenta => codes.push("95".to_string()),
                Color::BrightCyan => codes.push("96".to_string()),
                Color::BrightWhite => codes.push("97".to_string()),
                Color::RGB(r, g, b) => {
                    codes.push("38".to_string());
                    codes.push("2".to_string());
                    codes.push(r.to_string());
                    codes.push(g.to_string());
                    codes.push(b.to_string());
                }
            }
        }

        // 添加背景色代码
        if let Some(bg) = self.background {
            match bg {
                Color::Black => codes.push("40".to_string()),
                Color::Red => codes.push("41".to_string()),
                Color::Green => codes.push("42".to_string()),
                Color::Yellow => codes.push("43".to_string()),
                Color::Blue => codes.push("44".to_string()),
                Color::Magenta => codes.push("45".to_string()),
                Color::Cyan => codes.push("46".to_string()),
                Color::White => codes.push("47".to_string()),
                Color::BrightBlack => codes.push("100".to_string()),
                Color::BrightRed => codes.push("101".to_string()),
                Color::BrightGreen => codes.push("102".to_string()),
                Color::BrightYellow => codes.push("103".to_string()),
                Color::BrightBlue => codes.push("104".to_string()),
                Color::BrightMagenta => codes.push("105".to_string()),
                Color::BrightCyan => codes.push("106".to_string()),
                Color::BrightWhite => codes.push("107".to_string()),
                Color::RGB(r, g, b) => {
                    codes.push("48".to_string());
                    codes.push("2".to_string());
                    codes.push(r.to_string());
                    codes.push(g.to_string());
                    codes.push(b.to_string());
                }
            }
        }

        if codes.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", codes.join(";"))
        }
    }
}

/// 样式化文本
#[derive(Debug, Clone)]
pub struct StyledText {
    text: String,
    style: Style,
}

impl Display for StyledText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut codes = Vec::new();
        
        // 添加前景色代码
        if let Some(fg) = self.style.foreground {
            match fg {
                Color::Black => codes.push("30".to_string()),
                Color::Red => codes.push("31".to_string()),
                Color::Green => codes.push("32".to_string()),
                Color::Yellow => codes.push("33".to_string()),
                Color::Blue => codes.push("34".to_string()),
                Color::Magenta => codes.push("35".to_string()),
                Color::Cyan => codes.push("36".to_string()),
                Color::White => codes.push("37".to_string()),
                Color::BrightBlack => codes.push("90".to_string()),
                Color::BrightRed => codes.push("91".to_string()),
                Color::BrightGreen => codes.push("92".to_string()),
                Color::BrightYellow => codes.push("93".to_string()),
                Color::BrightBlue => codes.push("94".to_string()),
                Color::BrightMagenta => codes.push("95".to_string()),
                Color::BrightCyan => codes.push("96".to_string()),
                Color::BrightWhite => codes.push("97".to_string()),
                Color::RGB(r, g, b) => {
                    codes.push(format!("38;2;{};{};{}", r, g, b));
                }
            }
        }

        // 添加背景色代码
        if let Some(bg) = self.style.background {
            match bg {
                Color::Black => codes.push("40".to_string()),
                Color::Red => codes.push("41".to_string()),
                Color::Green => codes.push("42".to_string()),
                Color::Yellow => codes.push("43".to_string()),
                Color::Blue => codes.push("44".to_string()),
                Color::Magenta => codes.push("45".to_string()),
                Color::Cyan => codes.push("46".to_string()),
                Color::White => codes.push("47".to_string()),
                Color::BrightBlack => codes.push("100".to_string()),
                Color::BrightRed => codes.push("101".to_string()),
                Color::BrightGreen => codes.push("102".to_string()),
                Color::BrightYellow => codes.push("103".to_string()),
                Color::BrightBlue => codes.push("104".to_string()),
                Color::BrightMagenta => codes.push("105".to_string()),
                Color::BrightCyan => codes.push("106".to_string()),
                Color::BrightWhite => codes.push("107".to_string()),
                Color::RGB(r, g, b) => {
                    codes.push(format!("48;2;{};{};{}", r, g, b));
                }
            }
        }

        // 添加其他样式代码
        if self.style.bold { codes.push("1".to_string()); }
        if self.style.italic { codes.push("3".to_string()); }
        if self.style.underline { codes.push("4".to_string()); }
        if self.style.strikethrough { codes.push("9".to_string()); }
        if self.style.dim { codes.push("2".to_string()); }
        if self.style.reverse { codes.push("7".to_string()); }
        if self.style.blink { codes.push("5".to_string()); }
        if self.style.rapid_blink { codes.push("6".to_string()); }
        if self.style.hidden { codes.push("8".to_string()); }
        if self.style.framed { codes.push("51".to_string()); }
        if self.style.encircled { codes.push("52".to_string()); }
        if self.style.overlined { codes.push("53".to_string()); }
        if self.style.superscript { codes.push("73".to_string()); }
        if self.style.subscript { codes.push("74".to_string()); }

        // 构建ANSI转义序列
        let codes_str = codes.join(";");
        write!(f, "\x1b[{}m{}\x1b[0m", codes_str, self.text)
    }
}

/// 样式化trait
pub trait Styled {
    fn style(self, style: Style) -> StyledText;
}

impl Styled for &str {
    fn style(self, style: Style) -> StyledText {
        StyledText {
            text: self.to_string(),
            style,
        }
    }
}

impl Styled for String {
    fn style(self, style: Style) -> StyledText {
        StyledText {
            text: self,
            style,
        }
    }
} 