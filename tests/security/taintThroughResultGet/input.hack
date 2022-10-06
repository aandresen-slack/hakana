abstract class Result<+T, +TErr> {
	abstract public function get(): T;
}

final class ResultSuccess<+T> extends Result<T, nothing> {
	public function __construct(private T $t) {}
	public function get(): T {
		return $this->t;
	}
}

final class ResultError extends Result<nothing, string> {
	public function __construct(private string $message) {}
    public function get(): T {
		throw new \Exception('bad');
	}
}

function bar(): void {
    foo($_GET['arr']);
}

function foo(shape('a' => string) $args): void {
    $a = get_a_result($args);
    if ($a is ResultSuccess<_>) {
        echo $a->get();
    }
}

function get_a_result(shape('a' => string) $args): Result<string> {
    return new ResultSuccess($args['a']);
}