use forge_domain::ToolCallService;
use insta::assert_snapshot;
use tempfile::TempDir;
use tokio::fs;
use crate::test_utils::setup_test_env;

use super::super::{Outline, OutlineInput};

#[tokio::test]
async fn tsx_outline() {
    let temp_dir = TempDir::new().unwrap();
    let environment = setup_test_env(&temp_dir).await;

    let content = r#"
interface Props {
    name: string;
    age: number;
}

function UserProfile({ name, age }: Props) {
    return (
        <div>
            <h1>{name}</h1>
            <p>Age: {age}</p>
        </div>
    );
}

const UserList: React.FC<{ users: Props[] }> = ({ users }) => {
    return (
        <ul>
            {users.map(user => (
                <UserProfile key={user.name} {...user} />
            ))}
        </ul>
    );
};

export class UserContainer extends React.Component<Props, { loading: boolean }> {
    state = { loading: true };

    componentDidMount() {
        this.setState({ loading: false });
    }

    render() {
        return this.state 
          ? <div>Loading...</div> 
          : <UserProfile {...this.props} />;
    }
}"#;
    let file_path = temp_dir.path().join("test.tsx");
    fs::write(&file_path, content).await.unwrap();

    let outline = Outline::new(environment);
    let result = outline
        .call(OutlineInput { path: temp_dir.path().to_string_lossy().to_string() })
        .await
        .unwrap();

    assert_snapshot!("outline_tsx", result);
}