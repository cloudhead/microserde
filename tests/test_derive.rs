use microserde::{json, Deserialize, Serialize};
use std::path::PathBuf;

#[derive(PartialEq, Debug, Serialize, Deserialize)]
enum Tag {
    A,
    #[serde(rename = "renamedB")]
    B,
}

#[derive(PartialEq, Debug, Serialize, Deserialize)]
struct Example {
    x: String,
    t1: Tag,
    t2: Tag,
    n: Nested,
}

#[derive(PartialEq, Debug, Serialize, Deserialize)]
struct Nested {
    y: Option<Vec<String>>,
    z: Option<String>,
}

#[derive(PartialEq, Debug, Serialize, Deserialize)]
struct WithDefaults {
    required: String,
    #[serde(default)]
    count: usize,
    #[serde(default, rename = "items")]
    values: Vec<String>,
}

#[derive(PartialEq, Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct CommentId(String);

#[derive(PartialEq, Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct Tags(Vec<String>);

#[derive(PartialEq, Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct NestedWrapper(Nested);

#[derive(PartialEq, Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct Generic<T>(Vec<T>);

#[derive(PartialEq, Debug, Serialize, Deserialize)]
struct WithPath {
    path: PathBuf,
}

#[test]
fn test_de() {
    let j = r#" {"x": "X", "t1": "A", "t2": "renamedB", "n": {"y": ["Y", "Y"]}} "#;
    let actual: Example = json::from_str(j).unwrap();
    let expected = Example {
        x: "X".to_owned(),
        t1: Tag::A,
        t2: Tag::B,
        n: Nested {
            y: Some(vec!["Y".to_owned(), "Y".to_owned()]),
            z: None,
        },
    };
    assert_eq!(actual, expected);
}

#[test]
fn test_ser() {
    let example = Example {
        x: "X".to_owned(),
        t1: Tag::A,
        t2: Tag::B,
        n: Nested {
            y: Some(vec!["Y".to_owned(), "Y".to_owned()]),
            z: None,
        },
    };
    let actual = json::to_string(&example);
    let expected = r#"{"x":"X","t1":"A","t2":"renamedB","n":{"y":["Y","Y"],"z":null}}"#;
    assert_eq!(actual, expected);
}

#[test]
fn test_default() {
    let actual: WithDefaults = json::from_str(r#"{"required":"present"}"#).unwrap();
    let expected = WithDefaults {
        required: "present".to_owned(),
        count: 0,
        values: Vec::new(),
    };
    assert_eq!(actual, expected);
}

#[test]
fn test_default_present() {
    let actual: WithDefaults =
        json::from_str(r#"{"required":"present","count":3,"items":["a","b"]}"#).unwrap();
    let expected = WithDefaults {
        required: "present".to_owned(),
        count: 3,
        values: vec!["a".to_owned(), "b".to_owned()],
    };
    assert_eq!(actual, expected);
}

#[test]
fn test_transparent_tuple_struct() {
    let actual = json::to_string(&CommentId("c1".to_owned()));

    assert_eq!(actual, r#""c1""#);
    assert_eq!(
        json::from_str::<CommentId>(&actual).unwrap(),
        CommentId("c1".to_owned())
    );
}

#[test]
fn test_transparent_sequence() {
    let actual = json::to_string(&Tags(vec!["a".to_owned(), "b".to_owned()]));

    assert_eq!(actual, r#"["a","b"]"#);
    assert_eq!(
        json::from_str::<Tags>(&actual).unwrap(),
        Tags(vec!["a".to_owned(), "b".to_owned()])
    );
}

#[test]
fn test_transparent_generic() {
    let actual = json::to_string(&Generic(vec![1u32, 2]));

    assert_eq!(actual, r#"[1,2]"#);
    assert_eq!(
        json::from_str::<Generic<u32>>(&actual).unwrap(),
        Generic(vec![1, 2])
    );
}

#[test]
fn test_transparent_map() {
    let actual = json::to_string(&NestedWrapper(Nested {
        y: Some(vec!["Y".to_owned()]),
        z: Some("Z".to_owned()),
    }));

    assert_eq!(actual, r#"{"y":["Y"],"z":"Z"}"#);
    assert_eq!(
        json::from_str::<NestedWrapper>(&actual).unwrap(),
        NestedWrapper(Nested {
            y: Some(vec!["Y".to_owned()]),
            z: Some("Z".to_owned())
        })
    );
}

#[test]
fn test_pathbuf_as_string() {
    let actual = json::to_string(&WithPath {
        path: PathBuf::from("src/lib.rs"),
    });

    assert_eq!(actual, r#"{"path":"src/lib.rs"}"#);
    assert_eq!(
        json::from_str::<WithPath>(&actual).unwrap(),
        WithPath {
            path: PathBuf::from("src/lib.rs")
        }
    );
}
