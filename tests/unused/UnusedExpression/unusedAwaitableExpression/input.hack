function foo(): void {
    $a = bar(5);
}

async function bar(int $i): Awaitable<int> {
    return $i;
}