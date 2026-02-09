// wildcards.rs — UN*X wildcard pattern matching
// Converted from: myq2-original/qcommon/wildcards.c
// Original author: Florian Schintke (1996-2000)
//
// Supports: * (any chars), ? (single char), [...] (char sets), [!...] (negation)

/// Test if `wildcard` pattern matches `test` string.
pub fn wildcardfit(wildcard: &str, test: &str) -> bool {
    let w = wildcard.as_bytes();
    let t = test.as_bytes();
    wildcardfit_bytes(w, t)
}

fn wildcardfit_bytes(wildcard: &[u8], test: &[u8]) -> bool {
    let mut wi = 0;
    let mut ti = 0;
    let mut fit = true;

    while wi < wildcard.len() && fit && ti < test.len() {
        match wildcard[wi] {
            b'[' => {
                wi += 1; // skip opening bracket
                let result = set_match(wildcard, &mut wi, test, &mut ti);
                fit = result;
                // wi now points to closing ']', loop will increment
            }
            b'?' => {
                ti += 1;
            }
            b'*' => {
                let result = asterisk_match(wildcard, &mut wi, test, &mut ti);
                fit = result;
                // asterisk_match advances wi past the *, but the loop
                // will increment, so decrement to compensate
                wi = wi.saturating_sub(1);
            }
            ch => {
                fit = ch == test[ti];
                ti += 1;
            }
        }
        wi += 1;
    }

    // consume trailing asterisks
    while wi < wildcard.len() && wildcard[wi] == b'*' && fit {
        wi += 1;
    }

    fit && ti == test.len() && wi == wildcard.len()
}

/// Scan a character set [...] and return whether it matches.
/// `wi` points to the first char inside the brackets (after '[').
/// On return, `wi` points to the closing ']'.
/// `ti` is advanced by one if matched.
fn set_match(wildcard: &[u8], wi: &mut usize, test: &[u8], ti: &mut usize) -> bool {
    let mut fit = false;
    let mut negation = false;
    let mut at_beginning = true;

    if *wi < wildcard.len() && wildcard[*wi] == b'!' {
        negation = true;
        *wi += 1;
    }

    while *wi < wildcard.len() && (wildcard[*wi] != b']' || at_beginning) {
        if !fit {
            if wildcard[*wi] == b'-'
                && !at_beginning
                && *wi > 0
                && *wi + 1 < wildcard.len()
                && wildcard[*wi - 1] < wildcard[*wi + 1]
                && wildcard[*wi + 1] != b']'
            {
                if *ti < test.len()
                    && test[*ti] >= wildcard[*wi - 1]
                    && test[*ti] <= wildcard[*wi + 1]
                {
                    fit = true;
                    *wi += 1;
                }
            } else if *ti < test.len() && wildcard[*wi] == test[*ti] {
                fit = true;
            }
        }
        *wi += 1;
        at_beginning = false;
    }

    if negation {
        fit = !fit;
    }
    if fit && *ti < test.len() {
        *ti += 1;
    }

    fit
}

/// Handle '*' wildcard — skip characters in test until rest of pattern matches.
/// `wi` points to the '*'. On return, `wi` is advanced past the consumed pattern.
fn asterisk_match(wildcard: &[u8], wi: &mut usize, test: &[u8], ti: &mut usize) -> bool {
    // skip the leading asterisk
    *wi += 1;

    // consume leading ?s and *s
    while *ti < test.len()
        && *wi < wildcard.len()
        && (wildcard[*wi] == b'?' || wildcard[*wi] == b'*')
    {
        if wildcard[*wi] == b'?' {
            *ti += 1;
        }
        *wi += 1;
    }

    // consume remaining asterisks
    while *wi < wildcard.len() && wildcard[*wi] == b'*' {
        *wi += 1;
    }

    if *ti >= test.len() && *wi < wildcard.len() {
        return false;
    }
    if *ti >= test.len() && *wi >= wildcard.len() {
        return true;
    }

    // Wildcard exhausted but test remains — the * matches everything left
    if *wi >= wildcard.len() {
        *ti = test.len();
        return true;
    }

    // neither test nor wildcard are empty, first char of wildcard isn't [*?]
    let mut fit = true;
    if !wildcardfit_bytes(&wildcard[*wi..], &test[*ti..]) {
        loop {
            *ti += 1;
            // skip non-matching chars in test
            while *ti < test.len()
                && wildcard[*wi] != test[*ti]
                && wildcard[*wi] != b'['
            {
                *ti += 1;
            }

            if *ti < test.len() {
                if wildcardfit_bytes(&wildcard[*wi..], &test[*ti..]) {
                    break;
                }
            } else {
                fit = false;
                break;
            }
        }
    }

    if *ti >= test.len() && *wi >= wildcard.len() {
        fit = true;
    }

    fit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(wildcardfit("hello", "hello"));
        assert!(!wildcardfit("hello", "world"));
    }

    #[test]
    fn test_question_mark() {
        assert!(wildcardfit("h?llo", "hello"));
        assert!(wildcardfit("?????", "hello"));
        assert!(!wildcardfit("h?llo", "hllo"));
    }

    #[test]
    fn test_asterisk() {
        assert!(wildcardfit("*", "anything"));
        assert!(wildcardfit("h*o", "hello"));
        assert!(wildcardfit("h*", "hello"));
        assert!(wildcardfit("*lo", "hello"));
        assert!(wildcardfit("*.txt", "file.txt"));
        assert!(!wildcardfit("*.txt", "file.doc"));
    }

    #[test]
    fn test_character_set() {
        assert!(wildcardfit("[hH]ello", "hello"));
        assert!(wildcardfit("[hH]ello", "Hello"));
        assert!(!wildcardfit("[hH]ello", "jello"));
    }

    #[test]
    fn test_negated_set() {
        assert!(wildcardfit("[!j]ello", "hello"));
        assert!(!wildcardfit("[!h]ello", "hello"));
    }

    #[test]
    fn test_range() {
        assert!(wildcardfit("[a-z]ello", "hello"));
        assert!(!wildcardfit("[a-z]ello", "Hello"));
    }

    #[test]
    fn test_empty() {
        assert!(wildcardfit("", ""));
        assert!(!wildcardfit("", "x"));
        assert!(!wildcardfit("x", ""));
        assert!(wildcardfit("*", ""));
    }

    #[test]
    fn test_complex() {
        assert!(wildcardfit("*.[ch]", "file.c"));
        assert!(wildcardfit("*.[ch]", "file.h"));
        assert!(!wildcardfit("*.[ch]", "file.o"));
        assert!(wildcardfit("test*.txt", "test123.txt"));
    }
}
