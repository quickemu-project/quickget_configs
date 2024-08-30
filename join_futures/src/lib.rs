use proc_macro::{TokenStream, TokenTree};

#[proc_macro]
pub fn join_futures(input: TokenStream) -> TokenStream {
    let mut tokens = input.into_iter();

    let variable = match tokens.find(|t| matches!(t, TokenTree::Ident(_))) {
        Some(TokenTree::Ident(variable)) => variable.to_string(),
        _ => panic!("You must provide a variable containing futures to join"),
    };
    let flatten_amount = match tokens.find(|t| matches!(t, TokenTree::Literal(_))) {
        Some(TokenTree::Literal(flatten_amount)) => flatten_amount.to_string().trim().parse::<usize>().unwrap(),
        _ => 0,
    };

    let tokens = tokens.skip_while(|t| matches!(t, TokenTree::Punct(_)));
    let value_type = match tokens.map(|t| t.to_string()).collect::<String>() {
        s if s.is_empty() => "Vec<Config>".to_string(),
        s => s,
    };

    let mut value = format!("futures::future::join_all({variable}).await");
    if flatten_amount > 0 {
        value.push_str(&format!(
            ".into_iter(){}.collect::<{value_type}>()",
            ".flatten()".repeat(flatten_amount),
        ));
    }
    value.parse().unwrap()
}
