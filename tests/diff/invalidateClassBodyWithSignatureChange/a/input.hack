final class A {
	public static function aa(): void {
        B::bb();
        C::cc();
    }
}

final class B {
	public static function bb(): void {
    }
}

final class C {
    public static function cc(): void {}
}