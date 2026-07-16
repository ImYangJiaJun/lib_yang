//! FieldType 使用示例
//!
//! 演示如何使用 FieldType 定义存储类型，以及如何为字段附加关系元数据

use yang_base::table::{Field, FieldType, RelationType};

fn main() {
    println!("=== FieldType 使用示例 ===\n");

    // 基本类型
    println!("1. 基本类型：");
    let name_type = FieldType::String { max_length: 50 };
    println!(
        "  - 姓名字段: {:?} ({})",
        name_type,
        name_type.display_name()
    );

    let age_type = FieldType::Integer;
    println!("  - 年龄字段: {:?} ({})", age_type, age_type.display_name());

    let balance_type = FieldType::Double;
    println!(
        "  - 余额字段: {:?} ({})",
        balance_type,
        balance_type.display_name()
    );

    let active_type = FieldType::Boolean;
    println!(
        "  - 激活状态: {:?} ({})",
        active_type,
        active_type.display_name()
    );

    // 时间类型
    println!("\n2. 时间类型：");
    let birth_date_type = FieldType::Date;
    println!(
        "  - 出生日期: {:?} ({})",
        birth_date_type,
        birth_date_type.display_name()
    );

    let created_at_type = FieldType::DateTime;
    println!(
        "  - 创建时间: {:?} ({})",
        created_at_type,
        created_at_type.display_name()
    );

    let updated_at_type = FieldType::Timestamp;
    println!(
        "  - 更新时间戳: {:?} ({})",
        updated_at_type,
        updated_at_type.display_name()
    );

    // 复杂类型
    println!("\n3. 复杂类型：");
    let description_type = FieldType::Text;
    println!(
        "  - 描述字段: {:?} ({})",
        description_type,
        description_type.display_name()
    );

    let metadata_type = FieldType::Json;
    println!(
        "  - 元数据字段: {:?} ({})",
        metadata_type,
        metadata_type.display_name()
    );

    // 枚举类型
    println!("\n4. 枚举类型：");
    let status_type = FieldType::Enum {
        values: vec![
            "pending".to_string(),
            "approved".to_string(),
            "rejected".to_string(),
        ],
    };
    println!(
        "  - 状态字段: {:?} ({})",
        status_type,
        status_type.display_name()
    );

    // 关系元数据与存储类型正交
    println!("\n5. 关联字段：");
    let _user_id_field = Field::bigint("user_id").relation("users", "id", RelationType::ManyToOne);
    println!("  - 用户ID字段: BigInt + ManyToOne(users.id)");

    // 类型检查
    println!("\n6. 类型检查：");
    println!("  - age_type 是数值类型: {}", age_type.is_numeric());
    println!("  - name_type 是数值类型: {}", name_type.is_numeric());
    println!(
        "  - created_at_type 是时间类型: {}",
        created_at_type.is_temporal()
    );
    println!("  - name_type 是文本类型: {}", name_type.is_text());
    println!(
        "  - description_type 是文本类型: {}",
        description_type.is_text()
    );
}
