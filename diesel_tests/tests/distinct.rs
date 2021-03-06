use super::schema::*;
use diesel::*;

#[test]
fn simple_distinct() {
    use schema::users::dsl::*;

    let connection = connection();
    connection
        .execute("INSERT INTO users (name) VALUES ('Sean'), ('Sean'), ('Tess')")
        .unwrap();

    let source = users.select(name).distinct();
    let expected_data = vec!["Sean".to_string(), "Tess".to_string()];
    let data: Vec<String> = source.load(&connection).unwrap();

    assert_eq!(expected_data, data);
}

#[cfg(feature = "postgres")]
#[test]
fn distinct_on() {
    use schema::users::dsl::*;

    let connection = connection();
    connection
        .execute(
            "INSERT INTO users (name, hair_color) VALUES ('Sean', 'black'), ('Sean', NULL), ('Tess', NULL), ('Tess', NULL)",
        )
        .unwrap();

    let source = users.select((name, hair_color)).distinct_on(name);
    let expected_data = vec![
        ("Sean".to_string(), Some("black".to_string())),
        ("Tess".to_string(), None),
    ];
    let data: Vec<_> = source.load(&connection).unwrap();

    assert_eq!(expected_data, data);
}
