function foo(): void {
    Vec\map(vec[1, 2, 3, 4], async $i ==> {
        return await bar($i);
    });
}

async function bar(int $i): Awaitable<int> {
    return 5;
}