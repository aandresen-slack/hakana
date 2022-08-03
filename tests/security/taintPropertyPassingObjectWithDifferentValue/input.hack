<<\Hakana\Immutable>>
class User {
    public string $id;
    public $name = "Luke";

    public function __construct(string $userId) {
        $this->id = $userId;
    }
}

class UserUpdater {
    public static function doDelete(AsyncMysqlConnection $conn, User $user) : void {
        self::deleteUser($conn, $user->name);
    }

    public static function deleteUser(AsyncMysqlConnection $conn, string $userId) : void {
        $conn->query("delete from users where user_id = " . $userId);
    }
}

$userObj = new User((string) $_GET["user_id"]);
UserUpdater::doDelete(new AsyncMysqlConnection(), $userObj);