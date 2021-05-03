
mod trash {
    use chrono::{NaiveDate};
    use std::str::FromStr;

    #[derive(Debug)]
    pub struct InvalidDateError;

    #[derive(Eq, PartialEq, Debug)]
    enum TrashType {
        Organic,
        Recycling,
        Paper,
        Miscellaneous
    }

    #[derive(Debug)]
    struct TrashDate {
        pub date: NaiveDate,
        pub trash_type: TrashType
    }

    impl TrashDate {
        pub fn new(trash_type: TrashType, date_string: &str) -> Result<Self, InvalidDateError> {
            let date = match NaiveDate::from_str(date_string) {
                Ok(d) => d,
                Err(_) => return Err(InvalidDateError),
            };

            Ok(TrashDate{
                date,
                trash_type
            })
        }
    }
}