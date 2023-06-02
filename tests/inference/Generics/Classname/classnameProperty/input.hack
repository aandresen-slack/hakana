final class Resource<T> {
	public function __construct(
		private classname<T> $t,
	) {}
}

class A {}

function expectsResourceA(Resource<A> $_res): void {}

function bar(): void {
	$a = new Resource(A::class);
	expectsResourceA($a);
}
