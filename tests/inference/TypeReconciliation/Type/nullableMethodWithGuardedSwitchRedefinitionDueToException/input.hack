class One {
    public function fooFoo(): void {}
}

class B {
    public function barBar(?One $one = null): void {
        $a = rand(0, 4);

        if ($one === null) {
            switch ($a) {
                case 4:
                    $one = new One();
                    break;

                default:
                    throw new \Exception("bad");
            }
        }

        $one->fooFoo();
    }
}