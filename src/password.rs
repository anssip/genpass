use rand::Rng;

pub fn generate() -> String {
    // Define character set
    let low_case = "abcdefghijklmnopqrstuvxyz".to_string();
    let up_case = "ABCDEFGHIJKLMNOPQRSTUVXYZ".to_string();
    let numbers = "0123456789".to_string();
    let special = "£$&()*+[]@#^-_!?".to_string();

    let mut password = "".to_string();

    for _ in 0..=14 {
        let char_group = random_index(4);

        password = match char_group {
            0 => append(&password, &low_case),
            1 => append(&password, &up_case),
            2 => append(&password, &numbers),
            3 => append(&password, &special),
            _ => password,
        }
    }

    password
}

fn random_index(range: usize) -> usize {
    let mut rng = rand::thread_rng();
    rng.gen_range(0..range.try_into().unwrap())
}

fn append(to: &String, charset: &String) -> String {
    let character = charset
        .chars()
        .nth(random_index(charset.len() - 1))
        .unwrap();
    let mut result = String::from(to);
    result.push(character);
    result
}