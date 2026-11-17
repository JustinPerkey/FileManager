-- Example library database for the Database Manager. Run on an empty SQLite
-- database; `example.toml` is the matching config.
PRAGMA foreign_keys = ON;

CREATE TABLE publisher (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, country TEXT);
CREATE TABLE author (id INTEGER PRIMARY KEY, first_name TEXT NOT NULL, last_name TEXT NOT NULL,
                     born INTEGER, internal_note TEXT);
CREATE TABLE book (
  id INTEGER PRIMARY KEY,
  title TEXT NOT NULL,
  isbn TEXT NOT NULL UNIQUE,
  author_id INTEGER NOT NULL REFERENCES author(id) ON DELETE CASCADE,
  publisher_id INTEGER REFERENCES publisher(id) ON DELETE RESTRICT,
  format TEXT NOT NULL DEFAULT 'paperback',
  pages INTEGER,
  price REAL,
  in_print BOOLEAN NOT NULL DEFAULT 1,
  audio_length INTEGER,
  summary TEXT,
  revision INTEGER NOT NULL DEFAULT 1);
CREATE TABLE genre (code TEXT PRIMARY KEY, name TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE book_genre (
  book_id INTEGER NOT NULL REFERENCES book(id) ON DELETE CASCADE,
  genre_code TEXT NOT NULL REFERENCES genre(code) ON DELETE CASCADE,
  PRIMARY KEY (book_id, genre_code)) WITHOUT ROWID;
CREATE TABLE contribution (
  book_id INTEGER NOT NULL REFERENCES book(id) ON DELETE CASCADE,
  author_id INTEGER NOT NULL REFERENCES author(id) ON DELETE CASCADE,
  role TEXT NOT NULL DEFAULT 'editor',
  position INTEGER,
  PRIMARY KEY (book_id, author_id)) WITHOUT ROWID;
CREATE TABLE member (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT,
                     active BOOLEAN NOT NULL DEFAULT 1);
CREATE TABLE loan (
  id INTEGER PRIMARY KEY,
  book_id INTEGER NOT NULL REFERENCES book(id) ON DELETE CASCADE,
  member_id INTEGER REFERENCES member(id) ON DELETE SET NULL,
  loan_period INTEGER NOT NULL DEFAULT 1209600,
  returned INTEGER NOT NULL DEFAULT 0);
CREATE TABLE audit_log (id INTEGER PRIMARY KEY, message TEXT NOT NULL);

INSERT INTO publisher (id, name, country) VALUES
  (1, 'Northwind Press', 'Canada'),
  (2, 'Harbour Books', NULL);
INSERT INTO author (id, first_name, last_name, born, internal_note) VALUES
  (1, 'Ada', 'Lindqvist', 1961, 'prefers email'),
  (2, 'Tomas', 'Okafor', 1975, NULL),
  (3, 'Mira', 'Castellan', 1988, NULL);
INSERT INTO book (id, title, isbn, author_id, publisher_id, format, pages, price, in_print,
                  audio_length, summary, revision) VALUES
  (1, 'The Quiet Harbour', '9780000000011', 1, 1, 'hardcover', 320, 24.5, 1, 27000, 'A novel.', 1),
  (2, 'Salt and Iron', '9780000000028', 2, 1, 'paperback', 210, 12.0, 1, NULL, NULL, 2),
  (3, 'Maps of Nowhere', '9780000000035', 3, 2, 'ebook', NULL, 7.99, 1, 18900, NULL, 1),
  (4, 'A Short Winter', '9780000000042', 1, NULL, 'paperback', 150, 9.5, 0, NULL, NULL, 3),
  (5, 'Lanterns', '9780000000059', 2, 2, 'hardcover', 410, 29.0, 1, NULL, 'Poems.', 1);
INSERT INTO genre (code, name) VALUES
  ('fiction', 'Fiction'),
  ('poetry', 'Poetry'),
  ('history', 'History'),
  ('sci-fi', 'Science fiction');
INSERT INTO book_genre (book_id, genre_code) VALUES
  (1, 'fiction'), (2, 'fiction'), (2, 'history'), (3, 'sci-fi'), (5, 'poetry');
INSERT INTO contribution (book_id, author_id, role, position) VALUES
  (1, 2, 'editor', NULL),
  (2, 3, 'translator', 1),
  (3, 1, 'illustrator', NULL);
INSERT INTO member (id, name, email, active) VALUES
  (1, 'Priya Raman', 'priya@example.org', 1),
  (2, 'Jonas Weber', NULL, 1),
  (3, 'Lena Fischer', 'lena@example.org', 0);
INSERT INTO loan (id, book_id, member_id, loan_period, returned) VALUES
  (1, 1, 1, 1209600, 0),
  (2, 2, 2, 604800, 1),
  (3, 3, NULL, 1209600, 0),
  (4, 5, 3, 2592000, 1);
INSERT INTO audit_log (id, message) VALUES
  (1, 'database created'),
  (2, 'seed data loaded');
