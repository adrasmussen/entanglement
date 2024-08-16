pub const SUBNAV: &str = r#"
.subnav {
    overflow: hidden;
    background-color: #2196F3;
}

.subnav span {
    float: left;
    display: block;
    color: black;
    text-align: center;
    padding: 14px 16px;
    text-decoration: none;
    font-size: 17px;
}

.subnav span:hover {
    background-color: #eaeaea;
    color: black;
}

.subnav input[type=text], input[type=submit] {
    float: left;
    padding: 6px;
    border: none;
    margin-top: 8px;
    margin-right: 16px;
    margin-left: 6px;
    font-size: 17px;
}"#;

pub const SIDEPANEL: &str = r#"
.sidepanel {
    height: 100%;
    position: fixed;
    z-index: 1;
    top: 0;
    right: 0;
    background-color: #e9e9e9;
    overflow-x: hidden;
    transition: 0.3s;
}

.sidepanel img {
    margin: auto;
    padding: 20px 20px;
    width: 360px;
    object-fit: contain;
}

.sidepanel span {
    display: block;
    padding: 20px 20px;
    color: black;
    font-size 17px;
}"#;

pub const SIDEPANEL_EXT: &str = "400px";
