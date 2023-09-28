## todoz
    * cancelled trades detection 
    * confirmation count on BTC may fail:
        ** happened with a transaction w/ many inputs & outputs **

    * make the program async
    * get_balance - panics when connectio refused



## ablities
    now we can:
        * read open trades
        * save open trades and update data about them
        * generate an address
        * send a message
        * check address balance on address provided
        * load api key from file
        * save index of used address
    
        * filter open trades based on AD ID - bot pickus up only those offers that are bot tradable ...  
    
        * minimum confirmation count 

        * display actual status in console
            - new trade added
            - general status of open trades
            - closed trade
            - trade cancelled

            * colors:
                BrightWhite <- new trade opened
                Red <- trade cancelled
                Green <- trade finalized
                Yellow <- Any other notes



