pub const TOPNAV: &str = r#"
.topnav {
    overflow: hidden;
    background-color: #e9e9e9;
}

.topnav a {
    float: left;
    display: block;
    color: black;
    text-align: center;
    padding: 14px 16px;
    text-decoration: none;
    font-size: 17px;
}

.topnav a:hover {
    background-color: #ddd;
    color: black;
}

.topnav a.active {
    background-color: #2196F3;
    color: white;
}
"#;

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

.subnav input[type=text], input[type=submit], button {
    float: left;
    padding: 6px;
    border: none;
    margin-top: 8px;
    margin-right: 8px;
    margin-left: 8px;
    font-size: 17px;
}

.subnav label {
    float: left;
    padding: 6px;
    border: none;
    margin-top: 8px;
    font-size: 17px;
}

.subnav input[type=checkbox] {
    float: left;
    margin-top: 17px;
}
"#;

pub const MODAL: &str = r#"
.modal {
    display: block;
    position: fixed;
    z-index: 1;
    left: 0;
    top: 0;
    width: 100%;
    height: 100%;
    overflow: auto;
    background-color: rgb(0,0,0);
    background-color: rgba(0,0,0,0.4);
}

.modal-content {
    background-color: #fefefe;
    margin: 5% auto;
    padding: 20px;
    border: 1px solid #888;
    width: fit-content;
}

.close {
    color: #aaa;
    float: right;
    font-size: 28px;
    font-weight: bold;
}

.close:hover,
.close:focus {
    color: black;
    text-decoration: none;
    cursor: pointer;
}

.modal-header {
    padding: 2px 16px;
    background-color: #2196F3;
    color: white;
}

.modal-footer {
    display: grid;
    padding: 2px 16px;
    background-color: #2196F3;
    color: white;
    text-align: center;
}

.modal-footer span {
    width: 600px;
}

.modal-media {
    display: grid;
    grid-template-columns: max-content max-content;
    grid-gap: 5px;
    padding: 10px 0px 10px 0px;
    height: fit-content;
    width: fit-content;
}

.modal-media img {
    float: left;
    height: 400px;
    object-fit: contain;
}

.modal-info {
    display: grid;
    grid-template-columns: max-content max-content;
    grid-gap: 10px;
    padding: 20px;
    color: black;
    font-size 17px;
}

.modal-info label {
    text-align: right;
}

.modal-info label:after {
    content: ":";
}

.modal-info textarea {
    width: 600px;
}

.modal-info input[type=submit], button {
    float: left;
    padding: 6px;
    border: none;
    margin-top: 8px;
    margin-right: 8px;
    margin-left: 8px;
    font-size: 15px;
}

.modal-info input[type=submit] {
    grid-column: 2;
}

.modal-body {
    display: grid;
    grid-gap: 5px;
    padding: 10px 0px 10px 0px;
    height: fit-content;
    width: min(stretch, fit-content);
}
"#;

pub const GALLERY_DETAIL: &str = r#"
.gallery-outer {
    display: grid;
    grid-template-columns: 400px 1fr 1fr;
    margin-top: 8px;
    margin-right: 8px;
    margin-left: 8px;
}

.gallery-media {
    width: 400px;
    object-fit: contain;
}

.gallery-info {
    display: grid;
    grid-template-columns: max-content max-content;
    grid-gap: 10px;
    padding: 20px;
    color: black;
    font-size 17px;
}

.gallery-info label {
    text-align: right;
}

.gallery-info label:after {
    content: ":";
}

.gallery-info textarea {
    width: 100%;
}

.gallery-info input[type=submit], button {
    padding: 6px;
    border: none;
    margin-top: 8px;
    margin-right: 8px;
    margin-left: 8px;
    font-size: 15px;
}

.gallery-info input[type=submit] {
    grid-column: 2;
}

.gallery-related {
    display: grid;
    grid-template-rows: repeat(2, auto auto);
    grid-gap: 10px;
    padding: 20px;
    color: black;
    font-size 17px;
}
"#;

pub const MEDIA_GRID: &str = r#"
.media-grid {
    display: grid;
    gap: 5px;
    grid-template-columns: repeat(auto-fit, minmax(400px, 1fr));
}

.media-tile {
    height: 300px;
    width: 300px;
    border: 5px solid #ffffff;
    display: flex;
    flex-direction: column;
}
"#;

pub const TABLE: &str = r#"
table {
    border-collapse: collapse;
    width: 100%;
}

td, th {
    border: 1px solid #ddd;
    padding: 8px;
}

tr:nth-child(even) {
    background-color: #f2f2f2
}

tr:hover {
    background-color: #ddd;
}

th {
    padding-top: 12px;
    padding-bottom: 12px;
    text-align: left;
    background-color: #04AA6D;
    color: white;
}
"#;

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
}
"#;

pub const SIDEPANEL_EXT: &str = "400px";
