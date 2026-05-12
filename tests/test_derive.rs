use microserde::{json, Deserialize, Serialize};

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
