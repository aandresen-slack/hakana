function get_name(): string {
    return $_GET["name"] ?? "unknown";
}

function get_dict(): dict<string, string> {
    return dict["name" => get_name()];
}

echo "<h1>" . get_dict()['name'] . "</h1>";