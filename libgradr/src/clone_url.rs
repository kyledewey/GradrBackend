extern crate url;

use self::url::Url;
use self::url::SchemeData::Relative;

pub struct CloneUrl {
    pub url: Url,
    username: String,
    project_name: String
}

fn is_clone_url(url: &Url) -> bool {
    fn string_ok(s: &str) -> bool {
        !s.contains_char(' ')
    }

    match &url.scheme_data {
        &Relative(ref data) => {
            let v = &data.path;
            (v.len() == 2 &&
             string_ok(v[0].as_slice()) &&
             string_ok(v[1].as_slice()) &&
             v[1].as_slice().ends_with(".git"))
        },
        _ => false
    }
}

impl CloneUrl {
    pub fn new_from_url(url: Url) -> Option<CloneUrl> {
        if is_clone_url(&url) {
            let op_user_proj =
                match &url.scheme_data {
                    &Relative(ref data) => {
                        let username = data.path[0].clone();
                        let trim: &[_] = &['.', 'g', 'i', 't'];
                        Some(
                            (data.path[0].clone(),
                             data.path[1].trim_right_chars(trim).to_string()))
                    },
                    _ => None
                };
            match op_user_proj {
                Some((username, project_name)) => {
                    Some(
                        CloneUrl {
                            url: url,
                            username: username,
                            project_name: project_name
                        })
                },
                _ => None
            }
        } else { None }
    }

    pub fn new_from_str(from: &str) -> Option<CloneUrl> {
        Url::parse(from).ok().and_then(|url| {
            CloneUrl::new_from_url(url)
        })
    }

    pub fn username(&self) -> &str {
        self.username.as_slice()
    }

    pub fn project_name(&self) -> &str {
        self.project_name.as_slice()
    }
}
