function foo() : string {
    $a = null;

    try {
        $a = dangerous();
    } catch (Exception $e) {
        return $a;
    }

    return $a;
}

function dangerous() : string {
    if (rand(0, 1)) {
        throw new \Exception("bad");
    }
    return "hello";
}