<<\Hakana\SecurityAnalysis\Specialize>>
class Unsafe {
    public function isUnsafe() {
        return $_GET["unsafe"];
    }
}
echo (new Unsafe())->isUnsafe();