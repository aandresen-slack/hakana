class Foo {
    public vec<mixed> $a = dict[];
    public vec<mixed> $b = dict[];
    public vec<mixed> $c = dict[];

    public function five(): bool {
        $has_changes = false;

        while ($this->a || ($this->b && $this->c)) {
            $has_changes = true;
            $this->alter();
        }

        return $has_changes;
    }

    public function alter() : void {
        if (rand(0, 1)) {
            array_pop($this->a);
        } else if (rand(0, 1)) {
            array_pop($this->a);
        } else {
            array_pop($this->c);
        }
    }
}