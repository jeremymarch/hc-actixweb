# Hoplite Challenge

This web application was modeled after the Hoplite Challenge, an ancient Greek verb conjugation game, which was conceived at the Latin/Greek Institute almost 50 years ago.  


## Installation

1. Install Rust  
    - curl --proto '=https' --tlsv1.3 https://sh.rustup.rs -sSf | sh
2. clone this repository
3. Set environment variables for the database connection and the key.  e.g.  
    - export HOPLITE_DB=postgres://username:password@localhost/dbname
    - export HCKEY=56d520157194bdab7aec18755508bf6d063be7a203ddb61ebaa203eb1335c2ab3c13ecba7fc548f4563ac1d6af0b94e6720377228230f210ac51707389bf3285
4. run unit tests with:
    - cargo test
5. compile and run the web server with:
    - cargo run
6. the Hoplite Challenge web application can now be opened in a web browser at http://0.0.0.0:8088


## Playing Hoplite Challenge

1. create a user account (or two if testing two-player mode)
2. login
3. create a new two-player or practice game by clicking "New" and filling out the desired options.  Now click Create Game.  
4. select the new game
5. a starting form will presented in the upper panel.  When you click "Go" you will be asked to change the starting form to reflect the new parameters.
