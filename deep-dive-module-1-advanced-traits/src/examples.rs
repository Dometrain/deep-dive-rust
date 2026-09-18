//! Standalone teaching snippets for *Deep Dive: Rust* Module 1 ("Advanced
//! Traits and Generics"), one submodule per code sample in the lesson.
//!
//! These are deliberately self-contained -- each submodule defines its own
//! tiny `Task`/trait set rather than reusing `crate::models` -- so a learner
//! can lift any single submodule out of this file and it still compiles on
//! its own. The *applied* version of these ideas, wired into the real HTTP
//! API and backed by Postgres, lives in [`crate::models`] instead.

/// 1.1 #1 -- Trait with Associated Type.
///
/// `type Item` lets `Collection` describe "a collection of some item type"
/// without a generic parameter on the trait itself; the *implementer*
/// (`VecCollection<T>`) is what pins `Item` down to a concrete type.
pub mod associated_types {
    pub trait Collection {
        type Item;

        fn add(&mut self, item: Self::Item);
        fn get(&self, index: usize) -> Option<&Self::Item>;
    }

    pub struct VecCollection<T> {
        items: Vec<T>,
    }

    impl<T> Default for VecCollection<T> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<T> VecCollection<T> {
        pub fn new() -> Self {
            Self { items: Vec::new() }
        }
    }

    impl<T> Collection for VecCollection<T> {
        type Item = T;

        fn add(&mut self, item: T) {
            self.items.push(item);
        }

        fn get(&self, index: usize) -> Option<&T> {
            self.items.get(index)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn stores_and_retrieves_items() {
            let mut collection = VecCollection::new();
            collection.add("Learn Rust");
            assert_eq!(collection.get(0), Some(&"Learn Rust"));
            assert_eq!(collection.get(1), None);
        }
    }
}

/// 1.1 #2 -- Implementing Associated Types for a real-shaped type.
pub mod task_list {
    pub trait TaskList {
        type Task;

        fn add_task(&mut self, task: Self::Task);
        fn list_tasks(&self) -> Vec<&Self::Task>;
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct Task {
        pub id: u32,
        pub title: String,
    }

    pub struct TodoList {
        tasks: Vec<Task>,
    }

    impl Default for TodoList {
        fn default() -> Self {
            Self::new()
        }
    }

    impl TodoList {
        pub fn new() -> Self {
            Self { tasks: Vec::new() }
        }
    }

    impl TaskList for TodoList {
        type Task = Task;

        fn add_task(&mut self, task: Task) {
            self.tasks.push(task);
        }

        fn list_tasks(&self) -> Vec<&Task> {
            self.tasks.iter().collect()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn lists_added_tasks_in_order() {
            let mut list = TodoList::new();
            list.add_task(Task {
                id: 1,
                title: "Learn Rust".into(),
            });
            list.add_task(Task {
                id: 2,
                title: "Ship it".into(),
            });

            let titles: Vec<&str> = list
                .list_tasks()
                .into_iter()
                .map(|t| t.title.as_str())
                .collect();
            assert_eq!(titles, vec!["Learn Rust", "Ship it"]);
        }
    }
}

/// 1.1 #2b -- Implementing the Standard Library's Own Associated-Type Trait.
///
/// `Iterator` is the associated-type trait every learner has already used
/// (every `for` loop drives one), so it's the most concrete possible proof
/// that "a trait with `type Item;`" isn't exotic. Implement its single
/// required method -- `fn next(&mut self) -> Option<Self::Item>` -- and the
/// whole adaptor/consumer toolkit (`map`, `filter`, `collect`, `sum`, ...)
/// comes for free, because those are default methods built on `next`.
pub mod implementing_iterator {
    /// Yields task ids from `current` up to (not including) `end`.
    pub struct TaskIdRange {
        pub current: u32,
        pub end: u32,
    }

    impl Iterator for TaskIdRange {
        // The associated type: this iterator commits, once, to yielding `u32`s.
        type Item = u32;

        fn next(&mut self) -> Option<Self::Item> {
            if self.current < self.end {
                let id = self.current;
                self.current += 1;
                Some(id) // more to come
            } else {
                None // signals the end -- `for`/`collect`/`sum` stop here
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_custom_next_drives_a_for_loop() {
            let mut seen = Vec::new();
            for id in (TaskIdRange { current: 1, end: 4 }) {
                seen.push(id);
            }
            assert_eq!(seen, vec![1, 2, 3]);
        }

        #[test]
        fn implementing_next_unlocks_the_whole_adaptor_toolkit_for_free() {
            // We wrote *only* `next`; `filter`, `map`, and `collect` are all
            // default methods `std` provides on top of it. This is the payoff
            // of implementing the associated-type trait rather than a
            // one-off loop.
            let even_ids: Vec<u32> = (TaskIdRange { current: 1, end: 8 })
                .filter(|id| id % 2 == 0)
                .collect();
            assert_eq!(even_ids, vec![2, 4, 6]);

            // And the idiomatic "all-or-first-error" trick: collecting an
            // iterator of `Result`s into a single `Result`, short-circuiting
            // on the first `Err`.
            let parsed: Result<Vec<u32>, _> =
                ["1", "2", "3"].iter().map(|s| s.parse::<u32>()).collect();
            assert_eq!(parsed, Ok(vec![1, 2, 3]));
        }
    }
}

/// 1.1 #3 -- Default Type Parameters.
///
/// `Displayable<T = String>` provides a default for `T`, so
/// `impl Displayable for Task` (no argument) means the same thing as
/// `impl Displayable<String> for Task`. A caller who needs a different
/// representation can implement `Displayable<u32>` for the *same* type
/// without conflicting with the default impl -- they're different traits
/// (`Displayable<String>` vs. `Displayable<u32>`) as far as the compiler is
/// concerned.
pub mod default_type_parameters {
    pub trait Displayable<T = String> {
        fn display(&self) -> T;
    }

    pub struct Task {
        pub id: u32,
        pub title: String,
    }

    impl Displayable for Task {
        fn display(&self) -> String {
            format!("Task {}: {}", self.id, self.title)
        }
    }

    impl Displayable<u32> for Task {
        fn display(&self) -> u32 {
            self.id
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn default_and_overridden_type_parameters_coexist() {
            let task = Task {
                id: 1,
                title: "Learn Rust".into(),
            };
            let summary: String = task.display();
            let id: u32 = task.display();

            assert_eq!(summary, "Task 1: Learn Rust");
            assert_eq!(id, 1);
        }
    }
}

/// 1.1 #4 -- Trait Bounds.
pub mod trait_bounds {
    use std::fmt::Display;

    pub struct Task {
        pub id: u32,
        pub title: String,
    }

    impl Display for Task {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Task {}: {}", self.id, self.title)
        }
    }

    /// `T: Display` is checked at compile time -- calling this with a type
    /// that doesn't implement `Display` is a compile error, not a runtime
    /// panic.
    pub fn print_displayable<T: Display>(item: T) -> String {
        format!("{item}")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn formats_any_display_type() {
            let task = Task {
                id: 1,
                title: "Learn Rust".into(),
            };
            assert_eq!(print_displayable(task), "Task 1: Learn Rust");
            assert_eq!(print_displayable(42), "42");
        }
    }
}

/// 1.1 #5 -- Refactoring the To-Do App: shared behaviour via traits.
///
/// This is the pattern applied for real in `crate::models::traits`:
/// `Displayable` and `Storable` are implemented for the actual `Task` type
/// used by the HTTP API, and `db::queries::create_task` logs through a
/// `T: Displayable + Storable` bound instead of a `Task`-specific function.
pub mod refactor_todo {
    pub trait Displayable {
        fn display(&self) -> String;
    }

    pub trait Storable {
        type Id;

        fn id(&self) -> Self::Id;
    }

    pub struct Task {
        pub id: u32,
        pub title: String,
    }

    impl Displayable for Task {
        fn display(&self) -> String {
            format!("Task {}: {}", self.id, self.title)
        }
    }

    impl Storable for Task {
        type Id = u32;

        fn id(&self) -> u32 {
            self.id
        }
    }

    pub fn print_and_store<T: Displayable + Storable>(item: T) -> (String, T::Id) {
        (item.display(), item.id())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn works_for_any_displayable_storable_type() {
            let task = Task {
                id: 1,
                title: "Learn Rust".into(),
            };
            let (summary, id) = print_and_store(task);
            assert_eq!(summary, "Task 1: Learn Rust");
            assert_eq!(id, 1);
        }
    }
}

/// 1.2 #1 -- Newtype Pattern: zero-cost wrappers that prevent mixing units.
pub mod newtype {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Meters(pub u32);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Seconds(pub u32);

    impl Meters {
        // A real implementation would prefer `impl std::ops::Add for Meters`
        // so `+` works directly; named as `add` here to match the lesson.
        #[allow(clippy::should_implement_trait)]
        pub fn add(self, other: Meters) -> Meters {
            Meters(self.0 + other.0)
        }
    }

    // Meters(10).add(Seconds(5)) does not compile: `add` only accepts
    // another `Meters`, so mixing units is a compile error, not a bug
    // waiting to happen at runtime.

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn adds_same_unit_wrappers() {
            assert_eq!(Meters(10).add(Meters(5)), Meters(15));
        }
    }
}

/// 1.2 #2 -- Supertraits: `Validatable` can only be implemented by types
/// that already implement `Display` and `Debug`.
pub mod supertraits {
    use std::fmt::{Debug, Display};

    pub trait Validatable: Display + Debug {
        fn validate(&self) -> Result<(), String>;
    }

    #[derive(Debug)]
    pub struct Task {
        pub id: u32,
        pub title: String,
    }

    impl Display for Task {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Task {}: {}", self.id, self.title)
        }
    }

    impl Validatable for Task {
        fn validate(&self) -> Result<(), String> {
            if self.title.is_empty() {
                Err(String::from("Title cannot be empty"))
            } else {
                Ok(())
            }
        }
    }

    pub fn validate_and_print<T: Validatable>(item: T) -> String {
        match item.validate() {
            Err(e) => format!("Validation error: {e}"),
            Ok(()) => format!("Valid: {item}"),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn reports_valid_and_invalid_tasks() {
            let valid = Task {
                id: 1,
                title: "Learn Rust".into(),
            };
            let invalid = Task {
                id: 2,
                title: String::new(),
            };

            assert_eq!(validate_and_print(valid), "Valid: Task 1: Learn Rust");
            assert_eq!(
                validate_and_print(invalid),
                "Validation error: Title cannot be empty"
            );
        }
    }
}

/// 1.2 #3 -- Validation with Newtype: an invalid `NonEmptyString` simply
/// cannot exist, so `ValidatedTask` never needs to re-check its title.
pub mod validation_newtype {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct NonEmptyString(String);

    impl NonEmptyString {
        pub fn new(s: &str) -> Result<Self, String> {
            if s.is_empty() {
                Err(String::from("String cannot be empty"))
            } else {
                Ok(NonEmptyString(s.to_string()))
            }
        }
    }

    pub struct ValidatedTask {
        pub id: u32,
        pub title: NonEmptyString,
    }

    impl ValidatedTask {
        pub fn new(id: u32, title: &str) -> Result<Self, String> {
            Ok(ValidatedTask {
                id,
                title: NonEmptyString::new(title)?,
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn rejects_empty_titles() {
            assert!(ValidatedTask::new(1, "").is_err());
        }

        #[test]
        fn accepts_non_empty_titles() {
            let task = ValidatedTask::new(1, "Learn Rust").unwrap();
            assert_eq!(task.title, NonEmptyString::new("Learn Rust").unwrap());
        }
    }
}

/// 1.2 #4 -- Composing Traits: `TaskTraits` bundles three supertraits so
/// call sites write one bound instead of three.
///
/// Mirrors the traits from [`refactor_todo`] and [`supertraits`] above,
/// redeclared locally so this snippet compiles standalone.
pub mod composing_traits {
    use std::fmt::{Debug, Display};

    pub trait Displayable {
        fn display(&self) -> String;
    }

    pub trait Storable {
        type Id;

        fn id(&self) -> Self::Id;
    }

    pub trait Validatable: Display + Debug {
        fn validate(&self) -> Result<(), String>;
    }

    pub trait TaskTraits: Displayable + Storable + Validatable {}

    #[derive(Debug)]
    pub struct Task {
        pub id: u32,
        pub title: String,
    }

    impl Display for Task {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "Task {}: {}", self.id, self.title)
        }
    }

    impl Displayable for Task {
        fn display(&self) -> String {
            format!("Task {}: {}", self.id, self.title)
        }
    }

    impl Storable for Task {
        type Id = u32;

        fn id(&self) -> u32 {
            self.id
        }
    }

    impl Validatable for Task {
        fn validate(&self) -> Result<(), String> {
            if self.title.is_empty() {
                Err(String::from("Title cannot be empty"))
            } else {
                Ok(())
            }
        }
    }

    impl TaskTraits for Task {}

    pub fn process_task<T>(task: T) -> Vec<String>
    where
        T: TaskTraits,
        T::Id: Debug,
    {
        let mut log = vec![format!("Processing task: {}", task.display())];
        log.push(format!("ID: {:?}", task.id()));
        if let Err(e) = task.validate() {
            log.push(format!("Validation failed: {e}"));
        }
        log
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_single_bound_gives_access_to_all_three_traits() {
            let task = Task {
                id: 1,
                title: "Learn Rust".into(),
            };
            let log = process_task(task);
            assert_eq!(log, vec!["Processing task: Task 1: Learn Rust", "ID: 1"]);
        }
    }
}
