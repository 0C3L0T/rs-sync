use std::path::Path;

#[derive(Debug)]
pub enum FilePath<'a> {
    Local(&'a Path),

    Remote {
        user: Option<&'a str>,
        host: &'a str,
        path: &'a Path,
    },
}

impl<'a> FilePath<'a> {
    pub fn parse(input: &'a str) -> FilePath {
        if let Some(colon_pos) = input.find(':') {
            // split the path into "user@host" and "/path/to/file"
            let (user_host_part, path_part) = input.split_at(colon_pos);
            let path_part = &path_part[1..];

            if let Some(at_pos) = user_host_part.find('@') {
                let (user, host) = user_host_part.split_at(at_pos);
                let host = &host[1..];
                return FilePath::Remote {
                    user: Some(user),
                    host,
                    path: Path::new(path_part),
                };
            } else {
                return FilePath::Remote {
                    user: None,
                    host: user_host_part,
                    path: Path::new(path_part),
                };
            }
        }

        FilePath::Local(Path::new(input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_local_absolute_path() {
        let input = "/home/user/file.txt";
        let result = FilePath::parse(input);

        match result {
            FilePath::Local(path) => assert_eq!(path, Path::new("/home/user/file.txt")),
            _ => panic!("Expected a local path."),
        }
    }

    #[test]
    fn test_local_relative_path() {
        let input = "./file.txt";
        let result = FilePath::parse(input);

        match result {
            FilePath::Local(path) => assert_eq!(path, Path::new("./file.txt")),
            _ => panic!("Expected a local path."),
        }
    }

    #[test]
    fn test_remote_path() {
        let input = "user@hostname:/remote/path/file.txt";
        let result = FilePath::parse(input);

        match result {
            FilePath::Remote { user, host, path } => {
                assert_eq!(user, Some("user"));
                assert_eq!(host, "hostname");
                assert_eq!(path, Path::new("/remote/path/file.txt"));
            }
            _ => panic!("Expected a remote path."),
        }
    }

    #[test]
    fn test_remote_path_without_user() {
        let input = "hostname:/remote/path/file.txt";
        let result = FilePath::parse(input);

        match result {
            FilePath::Remote { user, host, path } => {
                assert_eq!(user, None);
                assert_eq!(host, "hostname");
                assert_eq!(path, Path::new("/remote/path/file.txt"));
            }
            _ => panic!("Expected a remote path."),
        }
    }

    #[test]
    fn test_remote_path_with_ipv4() {
        let input = "user@192.168.1.1:/remote/file.txt";
        let result = FilePath::parse(input);

        match result {
            FilePath::Remote { user, host, path } => {
                assert_eq!(user, Some("user"));
                assert_eq!(host, "192.168.1.1");
                assert_eq!(path, Path::new("/remote/file.txt"));
            }
            _ => panic!("Expected a remote path with an IPv4 address."),
        }
    }

    #[test]
    fn test_remote_path_with_escaped_spaces() {
        let input = "user@hostname:/remote/path/with\\ space/file.txt";
        let result = FilePath::parse(input);

        match result {
            FilePath::Remote { user, host, path } => {
                assert_eq!(user, Some("user"));
                assert_eq!(host, "hostname");
                assert_eq!(path, Path::new("/remote/path/with\\ space/file.txt"));
            }
            _ => panic!("Expected a remote path with escaped spaces."),
        }
    }

    #[test]
    fn test_invalid_remote_path_missing_colon() {
        let input = "user@hostname/remote/path/file.txt"; // No colon, should be local
        let result = FilePath::parse(input);

        match result {
            FilePath::Local(path) => {
                assert_eq!(path, Path::new("user@hostname/remote/path/file.txt"))
            }
            _ => panic!("Expected a local path."),
        }
    }

    #[test]
    fn test_empty_path() {
        let input = "";
        let result = FilePath::parse(input);

        match result {
            FilePath::Local(path) => assert_eq!(path, Path::new("")),
            _ => panic!("Expected an empty local path."),
        }
    }
}
