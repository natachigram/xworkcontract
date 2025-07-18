use cosmwasm_std::{DepsMut, Deps, StdResult, Order};
use crate::state::{CATEGORIES, SKILLS, CATEGORY_COUNTER, SKILL_COUNTER};

/// Get or create a category ID for a category name
pub fn get_or_create_category_id(deps: DepsMut, category_name: &str) -> StdResult<u8> {
    // First, check if category already exists
    for result in CATEGORIES.range(deps.storage, None, None, Order::Ascending) {
        let (id, name) = result?;
        if name.to_lowercase() == category_name.to_lowercase() {
            return Ok(id);
        }
    }
    
    // Category doesn't exist, create new one
    let next_id = CATEGORY_COUNTER.may_load(deps.storage)?.unwrap_or(1);
    if next_id == 255 {
        return Err(cosmwasm_std::StdError::generic_err("Maximum categories reached"));
    }
    
    CATEGORIES.save(deps.storage, next_id, &category_name.to_string())?;
    CATEGORY_COUNTER.save(deps.storage, &(next_id + 1))?;
    
    Ok(next_id)
}

/// Get or create skill IDs for skill names
pub fn get_or_create_skill_ids(deps: DepsMut, skill_names: &[String]) -> StdResult<Vec<u8>> {
    let mut skill_ids = Vec::new();
    
    for skill_name in skill_names {
        // Check if skill already exists
        let mut found_id = None;
        for result in SKILLS.range(deps.storage, None, None, Order::Ascending) {
            let (id, name) = result?;
            if name.to_lowercase() == skill_name.to_lowercase() {
                found_id = Some(id);
                break;
            }
        }
        
        let skill_id = if let Some(id) = found_id {
            id
        } else {
            // Skill doesn't exist, create new one
            let next_id = SKILL_COUNTER.may_load(deps.storage)?.unwrap_or(1);
            if next_id == 255 {
                continue; // Skip if we've reached max skills
            }
            SKILLS.save(deps.storage, next_id, skill_name)?;
            SKILL_COUNTER.save(deps.storage, &(next_id + 1))?;
            next_id
        };
        
        skill_ids.push(skill_id);
    }
    
    Ok(skill_ids)
}

/// Get category name by ID
pub fn get_category_name(deps: Deps, category_id: u8) -> StdResult<Option<String>> {
    CATEGORIES.may_load(deps.storage, category_id)
}

/// Get skill names by IDs
pub fn get_skill_names(deps: Deps, skill_ids: &[u8]) -> StdResult<Vec<String>> {
    let mut skill_names = Vec::new();
    
    for &skill_id in skill_ids {
        if let Some(skill_name) = SKILLS.may_load(deps.storage, skill_id)? {
            skill_names.push(skill_name);
        }
    }
    
    Ok(skill_names)
}

/// Calculate budget range from amount
pub fn calculate_budget_range(budget: cosmwasm_std::Uint128) -> u8 {
    let amount = budget.u128();
    if amount == 0 {
        0 // Free
    } else if amount < 500_000_000 { // < $500 (assuming 6 decimal places)
        1 // Low
    } else if amount < 5_000_000_000 { // < $5,000
        2 // Mid
    } else {
        3 // High
    }
}

/// Initialize default categories and skills
pub fn initialize_default_mappings(deps: DepsMut) -> StdResult<()> {
    // Default categories
    let default_categories = vec![
        "Web Development",
        "Mobile Development", 
        "Data Science",
        "Design & Creative",
        "Writing & Content",
        "Marketing & Sales",
        "Business & Strategy",
        "Engineering",
        "AI & Machine Learning",
        "Blockchain & Crypto",
        "DevOps & Infrastructure",
        "QA & Testing",
        "Legal & Compliance",
        "Finance & Accounting",
        "Customer Support",
        "Other",
    ];
    
    for (i, category) in default_categories.iter().enumerate() {
        let id = (i + 1) as u8;
        CATEGORIES.save(deps.storage, id, &category.to_string())?;
    }
    CATEGORY_COUNTER.save(deps.storage, &(default_categories.len() as u8 + 1))?;
    
    // Default skills
    let default_skills = vec![
        // Programming Languages
        "JavaScript", "Python", "TypeScript", "Java", "C++", "C#", "Go", "Rust", "PHP", "Ruby",
        "Swift", "Kotlin", "Solidity", "SQL", "R", "MATLAB", "Scala", "Perl", "Dart", "Lua",
        
        // Web Technologies
        "React", "Vue.js", "Angular", "Node.js", "Express", "Django", "Flask", "Laravel", 
        "Spring", "ASP.NET", "Next.js", "Nuxt.js", "HTML/CSS", "Bootstrap", "Tailwind CSS",
        
        // Mobile Development
        "React Native", "Flutter", "iOS Development", "Android Development", "Xamarin", "Ionic",
        
        // Databases
        "MongoDB", "PostgreSQL", "MySQL", "Redis", "Elasticsearch", "DynamoDB", "Cassandra",
        
        // Cloud & DevOps
        "AWS", "Google Cloud", "Azure", "Docker", "Kubernetes", "Terraform", "Jenkins", "GitLab CI",
        
        // Design & Creative
        "UI/UX Design", "Figma", "Adobe Creative Suite", "Sketch", "Canva", "3D Modeling", "Animation",
        
        // Data & AI
        "Machine Learning", "Deep Learning", "TensorFlow", "PyTorch", "Data Analysis", "Pandas", "NumPy",
        
        // Blockchain
        "Smart Contracts", "DeFi", "NFTs", "Web3", "Ethereum", "Bitcoin", "Polygon", "Cosmos",
        
        // Other
        "Project Management", "Agile", "Scrum", "Content Writing", "SEO", "Digital Marketing",
        "Social Media", "Video Editing", "Copywriting", "Translation", "Research", "Excel",
    ];
    
    for (i, skill) in default_skills.iter().enumerate() {
        let id = (i + 1) as u8;
        SKILLS.save(deps.storage, id, &skill.to_string())?;
    }
    SKILL_COUNTER.save(deps.storage, &(default_skills.len() as u8 + 1))?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::mock_dependencies;
    
    #[test]
    fn test_category_management() {
        let mut deps = mock_dependencies();
        
        // Test creating new category
        let category_id = get_or_create_category_id(deps.as_mut(), "Test Category").unwrap();
        assert_eq!(category_id, 1);
        
        // Test getting existing category
        let same_id = get_or_create_category_id(deps.as_mut(), "Test Category").unwrap();
        assert_eq!(same_id, category_id);
        
        // Test getting category name
        let name = get_category_name(deps.as_ref(), category_id).unwrap();
        assert_eq!(name, Some("Test Category".to_string()));
    }
    
    #[test]
    fn test_skill_management() {
        let mut deps = mock_dependencies();
        
        let skills = vec!["JavaScript".to_string(), "Python".to_string()];
        let skill_ids = get_or_create_skill_ids(deps.as_mut(), &skills).unwrap();
        assert_eq!(skill_ids.len(), 2);
        
        let retrieved_skills = get_skill_names(deps.as_ref(), &skill_ids).unwrap();
        assert_eq!(retrieved_skills, skills);
    }
    
    #[test]
    fn test_budget_range_calculation() {
        assert_eq!(calculate_budget_range(cosmwasm_std::Uint128::zero()), 0);
        assert_eq!(calculate_budget_range(cosmwasm_std::Uint128::new(100_000_000)), 1); // $100
        assert_eq!(calculate_budget_range(cosmwasm_std::Uint128::new(1_000_000_000)), 2); // $1,000
        assert_eq!(calculate_budget_range(cosmwasm_std::Uint128::new(10_000_000_000)), 3); // $10,000
    }
}
