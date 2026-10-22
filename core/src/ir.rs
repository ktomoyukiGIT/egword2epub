//! 中間表現（解析の結果の木）。Python 版 ir.py と同じ形

pub const OBJ: &str = "\u{FFFC}"; // 後置注記の検索で、割れない要素（注の参照）を表す一字

#[derive(Clone, Debug)]
pub enum Node {
    Text(String),
    Ruby(Vec<Node>, String),         // 親文字, ルビ
    Span(String, Vec<Node>),         // class, 中身
    NoteRef(String, String),         // 系統, 番号（半角）
    RangeMark { key: String, open: bool, cls: String },
}

pub fn plain(nodes: &[Node]) -> String {
    let mut out = String::new();
    for n in nodes {
        match n {
            Node::Text(s) => out.push_str(s),
            Node::Ruby(b, _) => out.push_str(&plain(b)),
            Node::Span(_, c) => out.push_str(&plain(c)),
            Node::NoteRef(..) => out.push_str(OBJ),
            Node::RangeMark { .. } => {}
        }
    }
    out
}

fn plen(n: &Node) -> usize { plain(std::slice::from_ref(n)).len() }

/// 素の文字列の位置 k で並びを二つに分ける（ルビと注の参照は割らない）
fn cut(nodes: &[Node], k: usize, down: bool) -> (Vec<Node>, Vec<Node>) {
    let mut left = Vec::new();
    let mut o = 0;
    for (i, n) in nodes.iter().enumerate() {
        let l = plen(n);
        if k <= o { return (left, nodes[i..].to_vec()); }
        if o + l <= k { left.push(n.clone()); o += l; continue; }
        match n {
            Node::Text(s) => {
                left.push(Node::Text(s[..k - o].to_string()));
                let mut right = vec![Node::Text(s[k - o..].to_string())];
                right.extend_from_slice(&nodes[i + 1..]);
                return (left, right);
            }
            Node::Span(cls, ch) => {
                let (l2, r2) = cut(ch, k - o, down);
                if !l2.is_empty() { left.push(Node::Span(cls.clone(), l2)); }
                let mut right = Vec::new();
                if !r2.is_empty() { right.push(Node::Span(cls.clone(), r2)); }
                right.extend_from_slice(&nodes[i + 1..]);
                return (left, right);
            }
            _ => {
                if down { return (left, nodes[i..].to_vec()); }
                left.push(n.clone());
                return (left, nodes[i + 1..].to_vec());
            }
        }
    }
    (left, Vec::new())
}

/// 素の文字列の範囲 [a, b) を Span(cls) で包む
pub fn wrap(nodes: &[Node], a: usize, b: usize, cls: &str) -> Vec<Node> {
    let mut o = 0;
    for (i, n) in nodes.iter().enumerate() {
        let l = plen(n);
        if l > 0 && o <= a && b <= o + l {
            match n {
                Node::Ruby(base, rt) => {
                    let mut out = nodes[..i].to_vec();
                    out.push(Node::Ruby(wrap(base, a - o, b - o, cls), rt.clone()));
                    out.extend_from_slice(&nodes[i + 1..]);
                    return out;
                }
                Node::Span(c, ch) => {
                    let mut out = nodes[..i].to_vec();
                    out.push(Node::Span(c.clone(), wrap(ch, a - o, b - o, cls)));
                    out.extend_from_slice(&nodes[i + 1..]);
                    return out;
                }
                _ => {}
            }
            break;
        }
        o += l;
    }
    let (mut left, rest) = cut(nodes, a, true);
    let llen = plain(&left).len();
    let (mid, right) = cut(&rest, b - llen, false);
    left.push(Node::Span(cls.to_string(), mid));
    left.extend(right);
    left
}

#[derive(Clone, Debug)]
pub enum Block {
    Para { nodes: Vec<Node>, style: String, tail: String, cls: String },
    Yoko { nodes: Vec<Node>, indent: i64, align: String, right: i64 },
    Blank,
    Div { cls: String, children: Vec<Block> },
    Sep { style: String, length: i64 },
    Figure { fname: String, alt: String, caption: Vec<Vec<Node>> },
    Missing { fname: String, caption: String },
    Heading { level: i64, nodes: Vec<Node>, style: String, anchor: String },
    TocMark,
}
