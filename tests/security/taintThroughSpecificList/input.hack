function one(string $in): void {
    $a = vec[$in, "b"];
    two($a);
}

function two((string, string) $a): void {
    list($b, $c) = $a;
    echo $b;
}

function foo(): void {
    one($_GET["b"]);
}