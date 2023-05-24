use std::collections::VecDeque;

use crate::tokenizer::tokenize;

pub fn parse(input: &String) -> Vec<(String, VecDeque<String>)> {
    let tokens_binding = tokenize(input);
    let mut tokens = tokens_binding.iter().peekable();

    let mut aggregated_tokens: Vec<VecDeque<String>> = vec![];
    let mut curr_command: VecDeque<String> = VecDeque::new();
    let mut commands: Vec<(String, VecDeque<String>)> = Vec::new();

    while let Some(token) = tokens.next() {
        if token == "&&" {
            aggregated_tokens.push(curr_command.clone());
            curr_command.clear()
        } else {
            curr_command.push_back(token.clone());
        }

        if tokens.peek().is_none() {
            aggregated_tokens.push(curr_command.clone());
            curr_command.clear()
        }
    }

    for mut vec_token in aggregated_tokens {
        commands.push((vec_token.pop_front().unwrap(), vec_token))
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_multiple_commands_when_split_by_and_operator() {
        //given
        let input = String::from("ls -a && ls");

        //when
        let commands = parse(&input);

        //then
        let first_command = &commands.get(0).unwrap().0;
        let first_command_args = commands.get(0).unwrap().1.get(0).unwrap();

        let second_command = &commands.get(1).unwrap().0;
        let second_command_args = commands.get(1).unwrap().1.get(0).is_none();

        assert_eq!(first_command, &String::from("ls"));
        assert_eq!(first_command_args, &String::from("-a"));

        assert_eq!(second_command, &String::from("ls"));
        assert_eq!(second_command_args, true);
    }

    #[test]
    fn return_single_command() {
        //given
        let input = String::from("ls -a");

        //when
        let commands = parse(&input);

        //then
        let command = &commands.get(0).unwrap().0;
        let args = commands.get(0).unwrap().1.get(0).unwrap();

        assert_eq!(command, &String::from("ls"));
        assert_eq!(args, &String::from("-a"));
    }
}
