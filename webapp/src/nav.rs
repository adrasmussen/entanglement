use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum AppPages {
    Home,
    Gallery,
    Albums,
    Library,
    Settings,
    Status,
}

#[derive(Clone, PartialEq, Props)]
pub struct AppPageProps {
    page_signal: Signal<AppPages>,
}

#[component]
pub fn AppMainPage(props: AppPageProps) -> Element {
    let page = props.page_signal.read();

    match *page {
        AppPages::Home => rsx! { div {"home page"} },
        AppPages::Gallery => rsx! { div {"gallery page"} },
        _ => rsx! { div{"failed to find page"} },
    }
}

#[derive(Clone, PartialEq, Props)]
struct AppNavBarButtonProps {
    display: String,
    page: AppPages,
    page_signal: Signal<AppPages>,
}

fn AppNavBarButton(mut props: AppNavBarButtonProps) -> Element {
    // TODO: this can panic, do handle appopriately
    let mut page_signal = props.page_signal;

    let change_page = move |_| *page_signal.write() = props.page;

    rsx! {
        if props.page == AppPages::Home {
            span {
                class: "active",
                onclick: change_page,
                "{props.display}",
            }
        } else {
            span {
                onclick: change_page,
                "{props.display}",
            }
        }
    }
}

#[derive(Clone, PartialEq, Props)]
pub struct AppNavBarProps {
    page_signal: Signal<AppPages>,
}

#[component]
pub fn AppNavBar(props: AppNavBarProps) -> Element {
    let page_signal = props.page_signal;

    let navbar_style = r#"
        .topnav {
            overflow: hidden;
            background-color: #e9e9e9;
        }

        .topnav span {
            float: left;
            display: block;
            color: black;
            text-align: center;
            padding: 14px 16px;
            text-decoration: none;
            font-size: 17px;
        }

        .topnav span:hover {
            background-color: #ddd;
            color: black;
        }

        .topnav span.active {
            background-color: #2196F3;
            color: white;
        }
    "#;

    rsx! {
        div {
            style { "{navbar_style}" },
            div {class: "topnav",
                AppNavBarButton {
                    display: String::from("Home"),
                    page: AppPages::Home,
                    page_signal: page_signal,
                },
                AppNavBarButton {
                    display: String::from("Gallery"),
                    page: AppPages::Gallery,
                    page_signal: page_signal,
                }
                AppNavBarButton {
                    display: String::from("Albums"),
                    page: AppPages::Albums,
                    page_signal: page_signal,
                }
                AppNavBarButton {
                    display: String::from("Library"),
                    page: AppPages::Library,
                    page_signal: page_signal,
                }
                AppNavBarButton {
                    display: String::from("Settings"),
                    page: AppPages::Settings,
                    page_signal: page_signal,
                }
                AppNavBarButton {
                    display: String::from("Server Status"),
                    page: AppPages::Status,
                    page_signal: page_signal,
                }
            }
        }
    }
}
