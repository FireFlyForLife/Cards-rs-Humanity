/**
 * \file `DeckBuilder.js`
 * 
 * \brief This file contains the a frontend for the card deck API
 * \dependson `CrsH-ServerAPI.js` `jquery-3.4.0.js` `signals.js`
 */

var currentDeck = null;
var currentSelected = null;

function selectDeck() {
    var deckName = $("#deckField").val();
    sendGetCardDeck(deckName, function(cardDeck){
        $("#deckName").text("Deck name: " + deckName);

        currentDeck = cardDeck;
        
        renderDeck();
    });
}

function renderDeck() {
    $("#cardDiv").html("");

    $.each(currentDeck.blackCards, function(i, val) {
        var cardId = val.id;
        $("<div></div>", {
            class : "blackCard",
            text: val.content,
            click: function() {
                var card = _getCardById(cardId);
                if(card != null) {
                    currentSelected = card;
                    $(".selectedCard").removeClass("selectedCard");
                    $(this).addClass("selectedCard");
                }
            }
          }).appendTo("#cardDiv");
    });
    $.each(currentDeck.whiteCards, function(i, val) {
        var cardId = val.id;
        $("<div></div>", {
            class : "whiteCard",
            text: val.content,
            click: function() {
                var card = _getCardById(cardId);
                if(card != null) {
                    currentSelected = card;
                    $(".selectedCard").removeClass("selectedCard");
                    $(this).addClass("selectedCard");
                }
            }
          }).appendTo("#cardDiv");
    });
}

function addWhiteCard() {
    var content = prompt("Please enter the content for the card:");
    if(content != null) {
        sendAddCard(currentDeck.deckName, content, true)
            .done(function(cardId) {
                currentDeck.whiteCards.push({content: content, id: cardId});
                renderDeck();
            })
            .fail(function(errorMessage) {
                console.error("Could not add white card, reason: ", errorMessage);
            });
    }
}

function addBlackCard() {
    var content = prompt("Please enter the content for the card:");
    if(content != null) {
        sendAddCard(currentDeck.deckName, content, false)
            .done(function(cardId) {
                currentDeck.blackCards.push({content: content, id: cardId});
                renderDeck();
            })
            .fail(function(errorMessage) {
                console.error("Could not add black card, reason: ", errorMessage);
            });
    }
}

function removeCard() {
    if(currentDeck == null) {
        return;
    }

    var selectedCardContent = $(".selectedCard").text();
    if(selectedCardContent == null) {
        console.warn("No card selected, so nothing will be removed!");
        return;
    }
    var selectedCardId = _getIdByCard(selectedCardContent);
    if(selectedCardId == null) {
        console.error("Cannot find the id of the card in the deck!");
    }
    
    sendDeleteCard(currentDeck.deckName, selectedCardId)
        .done(function() {
            $.each(currentDeck.blackCards, function(i, val) {
                if (val.id == selectedCardId) {
                    currentDeck.blackCards.splice(i, 1);
                    return false;
                }
            })
            $.each(currentDeck.whiteCards, function(i, val) {
                if (val.id == selectedCardId) {
                    currentDeck.whiteCards.splice(i, 1);
                    return false;
                }
            })

            renderDeck();
        })
        .fail(function(errorMessage){
            console.error("Could not remove card, reason: " + errorMessage);
        });
    
    
    

}

function _getCardById(cardId) {
    if(currentDeck == null)
        return null;

    var retVal = null;
    $.each(currentDeck.blackCards, function(i, val) {
        if (val.id == cardId) {
            retVal = val;
            return false;
        }
    })

    $.each(currentDeck.whiteCards, function(i, val) {
        if (val.id == cardId) {
            retVal = val;
            return false;
        }
    })

    return retVal;
}

function _getIdByCard(cardContent) {
    if(currentDeck == null){
        return null;
    }

    var retVal = null;
    $.each(currentDeck.blackCards, function(i, val) {
        if(val.content == cardContent) {
            retVal = val.id;
            return false;
        }
    })
    $.each(currentDeck.whiteCards, function(i, val) {
        if(val.content == cardContent) {
            retVal = val.id;
            return false;
        }
    })

    return retVal;
}