function gameOver(
    int $b0,
    int $b1,
    int $b2,
    int $b3,
    int $b4,
    int $b5,
    int $b6,
    int $b7,
    int $b8
): bool {
    if (($b0 === 1 && $b1 === 1 && $b2 === 1)
        || ($b3 === 1 && $b4 === 1 && $b5 === 1)
        || ($b6 === 1 && $b7 === 1 && $b8 === 1)
    ) {
        return true;
    }

    return false;
}