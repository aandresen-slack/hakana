$a = 5;

while (rand(0, 1)) {
    if (rand(0, 1)) {
        $a = 7;
        break;
    }

    $a = 3;
}

echo $a;